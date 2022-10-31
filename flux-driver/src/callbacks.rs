use flux_common::iter::IterExt;
use flux_desugar as desugar;
use flux_errors::FluxSession;
use flux_middle::{
    fhir::{self, ConstInfo},
    global_env::GlobalEnv,
    rustc,
};
use flux_syntax::surface;
use flux_typeck::{self as typeck, wf::Wf};
use rustc_driver::{Callbacks, Compilation};
use rustc_errors::ErrorGuaranteed;
use rustc_hir::{def::DefKind, def_id::LocalDefId};
use rustc_interface::{interface::Compiler, Queries};
use rustc_middle::ty::{
    query::{query_values, Providers},
    TyCtxt, WithOptConstParam,
};
use rustc_session::config::ErrorOutputType;
use typeck::invariants;

use crate::{
    collector::{IgnoreKey, Ignores, SpecCollector, Specs},
    mir_storage,
};

pub(crate) struct FluxCallbacks {
    full_compilation: bool,
    error_format: ErrorOutputType,
}

impl FluxCallbacks {
    pub(crate) fn new(full_compilation: bool) -> Self {
        FluxCallbacks { full_compilation, error_format: Default::default() }
    }
}

impl Callbacks for FluxCallbacks {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        assert!(config.override_queries.is_none());
        self.error_format = config.opts.error_format;
        config.override_queries = Some(|_, local, _| {
            local.mir_borrowck = mir_borrowck;
        });
    }

    fn after_analysis<'tcx>(
        &mut self,
        compiler: &Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        if compiler.session().has_errors().is_some() {
            return Compilation::Stop;
        }

        queries.global_ctxt().unwrap().peek_mut().enter(|tcx| {
            if !is_tool_registered(tcx) {
                return;
            }
            let sess = FluxSession::new(self.error_format, tcx.sess.parse_sess.clone_source_map());
            let _ = check_crate(tcx, &sess);
            sess.finish_diagnostics();
        });

        if self.full_compilation {
            Compilation::Continue
        } else {
            Compilation::Stop
        }
    }
}

fn check_crate(tcx: TyCtxt, sess: &FluxSession) -> Result<(), ErrorGuaranteed> {
    let mut specs = SpecCollector::collect(tcx, sess)?;

    // Ignore everything and go home
    if specs.ignores.contains(&IgnoreKey::Crate) {
        return Ok(());
    }

    let map = build_fhir_map(tcx, sess, &mut specs)?;
    check_wf(tcx, sess, &map)?;

    let mut genv = GlobalEnv::new(tcx, sess, map);
    // Assert behavior from Crate config
    // TODO(atgeller) rest of settings from crate config
    if let Some(crate_config) = specs.crate_config {
        let assert_behavior = crate_config.check_asserts;
        genv.register_assert_behavior(assert_behavior);
    }

    let ck = CrateChecker::new(&mut genv, specs.ignores);

    if ck.ignores.contains(&IgnoreKey::Crate) {
        return Ok(());
    }

    let crate_items = tcx.hir_crate_items(());
    let items = crate_items.items().map(|item| item.def_id.def_id);
    let impl_items = crate_items
        .impl_items()
        .map(|impl_item| impl_item.def_id.def_id);

    items
        .chain(impl_items)
        .try_for_each_exhaust(|def_id| ck.check_def(def_id))
}

struct CrateChecker<'genv, 'tcx> {
    genv: &'genv mut GlobalEnv<'genv, 'tcx>,
    ignores: Ignores,
}

impl<'genv, 'tcx> CrateChecker<'genv, 'tcx> {
    fn new(genv: &'genv mut GlobalEnv<'genv, 'tcx>, ignores: Ignores) -> Self {
        CrateChecker { genv, ignores }
    }

    fn is_assumed(&self, def_id: LocalDefId) -> bool {
        self.genv.map().assumed(def_id.to_def_id())
    }

    /// `is_ignored` transitively follows the `def_id` 's parent-chain to check if
    /// any enclosing mod has been marked as `ignore`
    fn is_ignored(&self, def_id: LocalDefId) -> bool {
        let parent_def_id = self.genv.tcx.parent_module_from_def_id(def_id);
        if parent_def_id == def_id {
            false
        } else {
            self.ignores.contains(&IgnoreKey::Module(parent_def_id))
                || self.is_ignored(parent_def_id)
        }
    }

    fn check_def(&self, def_id: LocalDefId) -> Result<(), ErrorGuaranteed> {
        if self.is_ignored(def_id) {
            return Ok(());
        }

        match self.genv.tcx.def_kind(def_id.to_def_id()) {
            DefKind::Fn | DefKind::AssocFn => self.check_fn(def_id),
            DefKind::Enum | DefKind::Struct => self.check_adt_invariants(def_id),
            _ => Ok(()),
        }
    }

    fn check_fn(&self, def_id: LocalDefId) -> Result<(), ErrorGuaranteed> {
        if self.is_assumed(def_id) {
            return Ok(());
        }

        let mir = unsafe { mir_storage::retrieve_mir_body(self.genv.tcx, def_id).body };

        // HACK(nilehmann) this will ignore any code generated by a macro. This is
        // a temporary workaround to allow `#[derive(PartialEq, Eq)]` and should be
        // removed.
        if mir.span.ctxt() > rustc_span::SyntaxContext::root() {
            return Ok(());
        }

        if flux_common::config::CONFIG.dump_mir {
            let mut w = std::io::BufWriter::new(std::io::stdout());
            rustc_middle::mir::pretty::write_mir_fn(
                self.genv.tcx,
                &mir,
                &mut |_, _| Ok(()),
                &mut w,
            )
            .unwrap();
        }

        let body =
            rustc::lowering::LoweringCtxt::lower_mir_body(self.genv.tcx, self.genv.sess, mir)?;

        typeck::check(self.genv, def_id.to_def_id(), &body)
    }

    fn check_adt_invariants(&self, def_id: LocalDefId) -> Result<(), ErrorGuaranteed> {
        let adt_def = self.genv.adt_def(def_id.to_def_id());
        if adt_def.is_opaque() {
            return Ok(());
        }
        invariants::check_invariants(self.genv, &adt_def)
    }
}

fn build_fhir_map(
    tcx: TyCtxt,
    sess: &FluxSession,
    specs: &mut Specs,
) -> Result<fhir::Map, ErrorGuaranteed> {
    let mut map = fhir::Map::default();

    let mut err: Option<ErrorGuaranteed> = None;

    // Register Consts
    for (def_id, const_sig) in std::mem::take(&mut specs.consts) {
        let did = def_id.to_def_id();
        let sym = def_id_symbol(tcx, def_id);
        map.insert_const(ConstInfo { def_id: did, sym, val: const_sig.val });
    }

    // Register UIFs
    err = std::mem::take(&mut specs.uifs)
        .into_iter()
        .try_for_each_exhaust(|uif_def| {
            let name = uif_def.name;
            let uif_def = desugar::resolve_uif_def(sess, uif_def)?;
            map.insert_uif(name.name, uif_def);
            Ok(())
        })
        .err()
        .or(err);

    // Register AdtDefs
    err = specs
        .structs
        .iter_mut()
        .try_for_each_exhaust(|(def_id, def)| {
            let refined_by = def.refined_by.as_ref().unwrap_or(surface::Params::DUMMY);
            let adt_def = desugar::desugar_adt_def(
                sess,
                &map,
                def_id.to_def_id(),
                refined_by,
                std::mem::take(&mut def.invariants),
                def.opaque,
            )?;
            map.insert_adt(*def_id, adt_def);
            Ok(())
        })
        .err()
        .or(err);
    err = specs
        .enums
        .iter_mut()
        .try_for_each_exhaust(|(def_id, def)| {
            let refined_by = def.refined_by.as_ref().unwrap_or(surface::Params::DUMMY);
            let adt_def = desugar::desugar_adt_def(
                sess,
                &map,
                def_id.to_def_id(),
                refined_by,
                std::mem::take(&mut def.invariants),
                false,
            )?;
            map.insert_adt(*def_id, adt_def);
            Ok(())
        })
        .err()
        .or(err);

    // Desugaring after this depends on the `fhir::Map` containing the information
    // collected before, so we bail out if there's any error at this point.
    if let Some(err) = err {
        return Err(err);
    }

    // Qualifiers
    err = std::mem::take(&mut specs.qualifs)
        .into_iter()
        .try_for_each_exhaust(|qualifier| {
            let qualifier = desugar::desugar_qualifier(sess, &map, qualifier)?;
            map.insert_qualifier(qualifier);
            Ok(())
        })
        .err()
        .or(err);

    // Variants
    err = std::mem::take(&mut specs.structs)
        .into_iter()
        .try_for_each_exhaust(|(def_id, struct_def)| {
            map.insert_struct(def_id, desugar::desugar_struct_def(tcx, sess, &map, struct_def)?);
            Ok(())
        })
        .err()
        .or(err);

    err = std::mem::take(&mut specs.enums)
        .into_iter()
        .try_for_each_exhaust(|(def_id, enum_def)| {
            map.insert_enum(def_id, desugar::desugar_enum_def(tcx, sess, &map, enum_def)?);
            Ok(())
        })
        .err()
        .or(err);

    // FnSigs
    let aliases = std::mem::take(&mut specs.aliases);
    err = std::mem::take(&mut specs.fns)
        .into_iter()
        .try_for_each_exhaust(|(def_id, spec)| {
            if let Some(fn_sig) = spec.fn_sig {
                let fn_sig = surface::expand::expand_sig(sess, &aliases, fn_sig)?;
                let fn_sig = desugar::desugar_fn_sig(tcx, sess, &map, def_id, fn_sig)?;
                map.insert_fn_sig(def_id, fn_sig, spec.assume);
            }
            Ok(())
        })
        .err()
        .or(err);

    if let Some(err) = err {
        Err(err)
    } else {
        Ok(map)
    }
}

fn check_wf(tcx: TyCtxt, sess: &FluxSession, map: &fhir::Map) -> Result<(), ErrorGuaranteed> {
    let wf = Wf::new(tcx, sess, map);

    let mut err: Option<ErrorGuaranteed> = None;

    for adt_def in map.adts() {
        err = wf.check_adt_def(adt_def).err().or(err);
    }

    for qualifier in map.qualifiers() {
        err = wf.check_qualifier(qualifier).err().or(err);
    }

    for struct_def in map.structs() {
        let refined_by = map.refined_by(struct_def.def_id).unwrap();
        err = wf.check_struct_def(refined_by, struct_def).err().or(err);
    }

    for enum_def in map.enums() {
        err = wf.check_enum_def(enum_def).err().or(err);
    }

    for (_, fn_sig) in map.fn_sigs() {
        err = wf.check_fn_sig(fn_sig).err().or(err);
    }

    if let Some(err) = err {
        Err(err)
    } else {
        Ok(())
    }
}

fn def_id_symbol(tcx: TyCtxt, def_id: LocalDefId) -> rustc_span::Symbol {
    let did = def_id.to_def_id();
    let def_path = tcx.def_path(did);
    if let Some(dp) = def_path.data.last() {
        if let rustc_hir::definitions::DefPathData::ValueNs(sym) = dp.data {
            return sym;
        }
    }
    panic!("def_id_symbol fails on {did:?}")
}

#[allow(clippy::needless_lifetimes)]
fn mir_borrowck<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> query_values::mir_borrowck<'tcx> {
    let body_with_facts = rustc_borrowck::consumers::get_body_with_borrowck_facts(
        tcx,
        WithOptConstParam::unknown(def_id),
    );
    // SAFETY: This is safe because we are feeding in the same `tcx` that is
    // going to be used as a witness when pulling out the data.
    unsafe {
        mir_storage::store_mir_body(tcx, def_id, body_with_facts);
    }
    let mut providers = Providers::default();
    rustc_borrowck::provide(&mut providers);
    let original_mir_borrowck = providers.mir_borrowck;
    original_mir_borrowck(tcx, def_id)
}

fn is_tool_registered(tcx: TyCtxt) -> bool {
    for attr in tcx.hir().krate_attrs() {
        if rustc_ast_pretty::pprust::attribute_to_string(attr) == "#![register_tool(flux)]" {
            return true;
        }
    }
    false
}
