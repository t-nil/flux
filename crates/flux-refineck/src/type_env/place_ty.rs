use std::{clone::Clone, fmt, ops::ControlFlow};

use flux_common::{iter::IterExt, tracked_span_bug};
use flux_infer::{
    infer::{ConstrReason, InferCtxt, InferCtxtAt},
    refine_tree::{AssumeInvariants, RefineCtxt},
};
use flux_middle::{
    global_env::GlobalEnv,
    rty::{
        fold::{FallibleTypeFolder, TypeFoldable, TypeVisitable, TypeVisitor},
        AdtDef, BaseTy, Binder, EarlyBinder, Expr, GenericArg, GenericArgsExt, List, Loc,
        Mutability, Path, PtrKind, Ref, Sort, Ty, TyKind, VariantIdx, VariantSig, FIRST_VARIANT,
    },
};
use flux_rustc_bridge::mir::{FieldIdx, Place, PlaceElem};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use rustc_hir::def_id::DefId;

use crate::{checker::errors::CheckerErrKind, CheckerConfig};

type CheckerResult<T = ()> = std::result::Result<T, CheckerErrKind>;

#[derive(Clone, Default)]
pub(crate) struct PlacesTree {
    map: FxHashMap<Loc, Binding>,
    loc_to_place: FxHashMap<Loc, Place>,
}

#[derive(Clone, Debug)]
pub(crate) struct Binding {
    pub kind: LocKind,
    pub ty: Ty,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub(crate) enum LocKind {
    Local,
    Box(Ty),
    Universal,
}

pub(crate) trait LookupKey {
    fn loc(&self) -> Loc;

    fn proj(&self) -> impl DoubleEndedIterator<Item = PlaceElem>;
}

impl LookupKey for Place {
    fn loc(&self) -> Loc {
        Loc::Local(self.local)
    }

    fn proj(&self) -> impl DoubleEndedIterator<Item = PlaceElem> {
        self.projection.iter().copied()
    }
}

impl LookupKey for Path {
    fn loc(&self) -> Loc {
        self.loc
    }

    fn proj(&self) -> impl DoubleEndedIterator<Item = PlaceElem> {
        self.projection().iter().map(|f| PlaceElem::Field(*f))
    }
}

pub(super) struct LookupResult<'a> {
    pub ty: Ty,
    pub is_strg: bool,
    pub is_constant_index: bool, // doesn't feel right...
    cursor: Cursor,
    bindings: &'a mut PlacesTree,
}

pub(crate) trait LookupMode {
    type Error = !;

    fn unpack(&mut self, ty: &Ty) -> Ty;

    fn downcast_struct(
        &mut self,
        adt: &AdtDef,
        args: &[GenericArg],
        idx: &Expr,
    ) -> Result<Vec<Ty>, Self::Error>;
}

struct Unfold<'a, 'infcx, 'genv, 'tcx>(&'a mut InferCtxt<'infcx, 'genv, 'tcx>);

impl LookupMode for Unfold<'_, '_, '_, '_> {
    type Error = CheckerErrKind;

    fn unpack(&mut self, ty: &Ty) -> Ty {
        self.0.hoister(AssumeInvariants::No).shallow().hoist(ty)
    }

    fn downcast_struct(
        &mut self,
        adt: &AdtDef,
        args: &[GenericArg],
        idx: &Expr,
    ) -> Result<Vec<Ty>, Self::Error> {
        downcast_struct(self.0, adt, args, idx)
    }
}

struct NoUnfold;

impl LookupMode for NoUnfold {
    fn downcast_struct(&mut self, _: &AdtDef, _: &[GenericArg], _: &Expr) -> Result<Vec<Ty>, !> {
        tracked_span_bug!("cannot unfold in `NoUnfold` mode")
    }

    fn unpack(&mut self, ty: &Ty) -> Ty {
        ty.clone()
    }
}

impl PlacesTree {
    pub(crate) fn unfold(
        &mut self,
        infcx: &mut InferCtxt,
        key: &impl LookupKey,
        checker_conf: CheckerConfig,
    ) -> CheckerResult {
        let cursor = self.cursor_for(key);
        Unfolder::new(infcx, cursor, checker_conf).run(self)
    }

    fn lookup_inner<M: LookupMode>(
        &mut self,
        key: &impl LookupKey,
        mut mode: M,
    ) -> Result<LookupResult, M::Error> {
        let mut cursor = self.cursor_for(key);
        let mut ty = self.get_loc(&cursor.loc).ty.clone();
        let mut is_strg = true;
        let mut is_constant_index = false;
        while let Some(elem) = cursor.next() {
            ty = mode.unpack(&ty);
            match elem {
                PlaceElem::Deref => {
                    match ty.kind() {
                        TyKind::Indexed(BaseTy::Adt(adt, args), _) if adt.is_box() => {
                            ty = args.box_args().0.clone();
                        }
                        TyKind::Indexed(BaseTy::RawPtr(deref_ty, _), _) => {
                            is_strg = false;
                            ty = deref_ty.clone();
                        }
                        TyKind::Ptr(_, path) => {
                            cursor.change_root(path);
                            ty = self.get_loc(&cursor.loc).ty.clone();
                        }
                        Ref!(_, deref_ty, _) => {
                            is_strg = false;
                            ty = deref_ty.clone();
                        }
                        TyKind::Uninit => {
                            ty = Ty::uninit();
                            break;
                        }
                        _ => tracked_span_bug!("invalid deref `{ty:?}`"),
                    }
                }
                PlaceElem::Field(f) => {
                    match ty.kind() {
                        TyKind::Indexed(BaseTy::Tuple(fields), _)
                        | TyKind::Indexed(BaseTy::Closure(_, fields, _), _)
                        | TyKind::Indexed(BaseTy::Coroutine(_, _, fields), _)
                        | TyKind::Downcast(.., fields) => {
                            ty = fields[f.as_usize()].clone();
                        }
                        TyKind::Indexed(BaseTy::Adt(adt, args), idx) => {
                            ty = mode.downcast_struct(adt, args, idx)?[f.as_usize()].clone();
                        }
                        _ => tracked_span_bug!("invalid field access `Field({f:?})` and `{ty:?}`"),
                    };
                }
                PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                    is_strg = false;
                    is_constant_index = matches!(elem, PlaceElem::ConstantIndex { .. });
                    match ty.kind() {
                        TyKind::Indexed(BaseTy::Array(array_ty, _), _) => {
                            ty = array_ty.clone();
                        }
                        TyKind::Indexed(BaseTy::Slice(slice_ty), _) => {
                            ty = slice_ty.clone();
                        }
                        _ => tracked_span_bug!("invalid index access `{ty:?}`"),
                    }
                }
                PlaceElem::Downcast(..) => {}
            }
        }
        cursor.reset();
        Ok(LookupResult { ty, is_strg, is_constant_index, cursor, bindings: self })
    }

    pub(crate) fn lookup_unfolding(
        &mut self,
        infcx: &mut InferCtxt,
        key: &impl LookupKey,
    ) -> CheckerResult<LookupResult> {
        self.lookup_inner(key, Unfold(infcx))
    }

    pub(crate) fn lookup(&mut self, key: &impl LookupKey) -> LookupResult {
        self.lookup_inner(key, NoUnfold).into_ok()
    }

    pub(crate) fn paths(&self) -> Vec<Path> {
        let mut paths = vec![];
        self.iter_flatten(|path, _, _| paths.push(path));
        paths
    }

    pub(crate) fn get(&self, path: &Path) -> Ty {
        let mut ty = &self.get_loc(&path.loc).ty;
        for f in path.projection() {
            match ty.kind() {
                TyKind::Downcast(.., fields)
                | TyKind::Indexed(BaseTy::Tuple(fields), _)
                | TyKind::Indexed(BaseTy::Closure(_, fields, _), _)
                | TyKind::Indexed(BaseTy::Coroutine(_, _, fields), _) => {
                    ty = &fields[f.as_usize()];
                }
                TyKind::Uninit => return Ty::uninit(),
                _ => tracked_span_bug!("{ty:?}"),
            }
        }
        ty.clone()
    }

    pub(crate) fn fmap_mut(&mut self, mut f: impl FnMut(&Ty) -> Ty) {
        self.try_fmap_mut::<!>(|ty| Ok(f(ty))).into_ok();
    }

    fn try_fmap_mut<E>(&mut self, mut f: impl FnMut(&Ty) -> Result<Ty, E>) -> Result<(), E> {
        self.map.values_mut().try_for_each(|binding| {
            binding.ty = f(&binding.ty)?;
            Ok(())
        })
    }

    pub(crate) fn flatten(self) -> Vec<(Path, LocKind, Ty)> {
        let mut bindings = vec![];
        self.iter_flatten(|path, kind, ty| bindings.push((path, kind.clone(), ty.clone())));
        bindings
    }

    pub(crate) fn insert(&mut self, loc: Loc, place: Place, kind: LocKind, ty: Ty) {
        self.map.insert(loc, Binding { kind, ty });
        self.loc_to_place.insert(loc, place);
    }

    fn remove(&mut self, loc: &Loc) -> Binding {
        self.map.remove(loc).unwrap_or_else(|| tracked_span_bug!())
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Loc, &Binding)> {
        self.map.iter()
    }

    fn iter_flatten(&self, mut f: impl FnMut(Path, &LocKind, &Ty)) {
        fn go(
            loc: &Loc,
            kind: &LocKind,
            ty: &Ty,
            proj: &mut Vec<FieldIdx>,
            f: &mut impl FnMut(Path, &LocKind, &Ty),
        ) {
            match ty.kind() {
                TyKind::Downcast(.., fields)
                | TyKind::Indexed(BaseTy::Tuple(fields), _)
                | TyKind::Indexed(BaseTy::Closure(_, fields, _), _)
                | TyKind::Indexed(BaseTy::Coroutine(_, _, fields), _) => {
                    for (idx, ty) in fields.iter().enumerate() {
                        proj.push(idx.into());
                        go(loc, kind, ty, proj, f);
                        proj.pop();
                    }
                }
                _ => f(Path::new(*loc, proj.as_slice()), kind, ty),
            }
        }
        for (loc, binding) in &self.map {
            go(loc, &binding.kind, &binding.ty, &mut vec![], &mut f);
        }
    }

    pub(crate) fn get_loc(&self, loc: &Loc) -> &Binding {
        self.map
            .get(loc)
            .unwrap_or_else(|| tracked_span_bug!("loc not found {loc:?}"))
    }

    fn get_loc_mut(&mut self, loc: &Loc) -> &mut Binding {
        self.map
            .get_mut(loc)
            .unwrap_or_else(|| tracked_span_bug!("loc not found {loc:?}"))
    }

    fn cursor_for(&self, key: &impl LookupKey) -> Cursor {
        let place = self.loc_to_place[&key.loc()].clone();
        Cursor::new(key, place)
    }

    pub(crate) fn path_to_place(&self, path: &Path) -> Place {
        let mut place = self.loc_to_place[&path.loc].clone();
        place.projection.extend(path.proj());
        place
    }
}

impl LookupResult<'_> {
    pub(crate) fn update(self, new: Ty) -> Ty {
        let old = self.ty.clone();
        Updater::update(self.bindings, self.cursor, |cursor, _| {
            if !cursor.is_exhausted() {
                // This means we are trying to do a strong updated inside an array/slice
                tracked_span_bug!("update through non-exhausted cursor");
            }
            new
        });
        old
    }

    pub(crate) fn unblock(self, rcx: &mut RefineCtxt, check_overflow: bool) {
        if self.ty.is_uninit() {
            return;
        }
        Updater::update(self.bindings, self.cursor, |_, ty| {
            // FIXME(nilehmann) in some situations we are calling unblock on a place that's not actually
            // blocked. This happens when taking a reference to an array or with a shared reborrow.
            // To avoid crashing, we call `Ty::unblocked` which would simply return the original
            // type if it wasn't unblocked. We should instead properly block the place in the first place
            // and ensure we are only unblocking blocked places.
            let mut unblocked = ty.unblocked();
            if self.is_strg {
                unblocked = rcx
                    .hoister(AssumeInvariants::yes(check_overflow))
                    .hoist(&unblocked);
            }
            unblocked
        });
    }

    pub(crate) fn block_with(self, new_ty: Ty) -> Ty {
        self.update(Ty::blocked(new_ty))
    }

    pub(crate) fn fold(self, infcx: &mut InferCtxtAt) -> CheckerResult<Ty> {
        let ty = fold(self.bindings, infcx, &self.ty, self.is_strg)?;
        self.update(ty.clone());
        Ok(ty)
    }

    pub(crate) fn path(&self) -> Path {
        self.cursor.to_path()
    }
}

struct Unfolder<'a, 'infcx, 'genv, 'tcx> {
    infcx: &'a mut InferCtxt<'infcx, 'genv, 'tcx>,
    insertions: Vec<(Loc, Place, Binding)>,
    cursor: Cursor,
    in_ref: Option<Mutability>,
    checker_conf: CheckerConfig,
    has_work: bool,
}

impl FallibleTypeFolder for Unfolder<'_, '_, '_, '_> {
    type Error = CheckerErrKind;

    fn try_fold_ty(&mut self, ty: &Ty) -> CheckerResult<Ty> {
        let Some(elem) = self.cursor.next() else {
            return self.unfold(ty);
        };
        let ty = self.unpack(ty);
        match elem {
            PlaceElem::Deref => self.deref(&ty),
            PlaceElem::Field(f) => self.field(&ty, f),
            PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                self.index(&ty)?;
                Ok(ty.clone())
            }
            PlaceElem::Downcast(_, variant) => self.downcast(&ty, variant)?.try_fold_with(self),
        }
    }
}

impl<'a, 'infcx, 'genv, 'tcx> Unfolder<'a, 'infcx, 'genv, 'tcx> {
    fn new(
        infcx: &'a mut InferCtxt<'infcx, 'genv, 'tcx>,
        cursor: Cursor,
        checker_conf: CheckerConfig,
    ) -> Self {
        Unfolder { infcx, cursor, insertions: vec![], in_ref: None, checker_conf, has_work: true }
    }

    fn run(mut self, bindings: &mut PlacesTree) -> CheckerResult {
        while self.should_continue() {
            let binding = bindings.get_loc_mut(&self.cursor.loc);
            binding.ty = binding.ty.try_fold_with(&mut self)?;
            for (loc, place, binding) in self.insertions.drain(..) {
                bindings.insert(loc, place, binding.kind, binding.ty);
            }
        }
        Ok(())
    }

    fn unfold(&mut self, ty: &Ty) -> CheckerResult<Ty> {
        if let TyKind::Indexed(BaseTy::Adt(adt, args), _) = ty.kind()
            && adt.is_box()
        {
            if self.in_ref.is_some() {
                Ok(ty.clone())
            } else {
                let (deref_ty, alloc) = args.box_args();
                let loc = self.unfold_box(deref_ty, alloc);
                Ok(Ty::ptr(PtrKind::Box, Path::from(loc)))
            }
        } else if let TyKind::StrgRef(re, path, deref_ty) = ty.kind() {
            assert!(self.in_ref.is_none());
            self.unfold_strg_ref(path, deref_ty);
            Ok(Ty::ptr(PtrKind::Mut(*re), path.clone()))
        } else if ty.is_struct() {
            let ty = self.unpack(ty);
            let ty = self.downcast(&ty, FIRST_VARIANT)?;
            Ok(ty)
        } else if ty.is_array() || ty.is_slice() {
            Ok(self.unpack(ty))
        } else {
            Ok(ty.clone())
        }
    }

    fn deref(&mut self, ty: &Ty) -> CheckerResult<Ty> {
        let ty = match ty.kind() {
            TyKind::StrgRef(re, path, ty) => {
                self.unfold_strg_ref(path, ty);
                Ty::ptr(PtrKind::Mut(*re), path.clone())
            }
            TyKind::Ptr(pk, path) => {
                self.change_root(path);
                Ty::ptr(*pk, path.clone())
            }
            TyKind::Indexed(BaseTy::Adt(adt, args), idx) if adt.is_box() => {
                let (deref_ty, alloc) = args.box_args();
                if self.in_ref.is_some() {
                    let args = List::from_arr([
                        GenericArg::Ty(deref_ty.try_fold_with(self)?),
                        GenericArg::Ty(alloc.clone()),
                    ]);
                    Ty::indexed(BaseTy::Adt(adt.clone(), args), idx.clone())
                } else {
                    let loc = self.unfold_box(deref_ty, alloc);
                    let path = Path::from(loc);
                    self.change_root(&path);
                    Ty::ptr(PtrKind::Box, path)
                }
            }
            Ref!(re, ty, mutbl) => {
                self.in_ref = self.in_ref.max(Some(*mutbl));
                Ty::mk_ref(*re, ty.try_fold_with(self)?, *mutbl)
            }
            _ => tracked_span_bug!("invalid deref of `{ty:?}`"),
        };
        Ok(ty)
    }

    fn unfold_strg_ref(&mut self, path: &Path, ty: &Ty) {
        let loc = path.to_loc().unwrap_or_else(|| tracked_span_bug!());
        let kind = match loc {
            Loc::Local(_) => LocKind::Local,
            Loc::Var(_) => LocKind::Universal,
        };
        let mut place = self.cursor.to_place();
        place.projection.push(PlaceElem::Deref);
        self.insertions
            .push((loc, place, Binding { kind, ty: ty.clone() }));
    }

    fn unfold_box(&mut self, deref_ty: &Ty, alloc: &Ty) -> Loc {
        let loc = Loc::from(self.infcx.define_var(&Sort::Loc));
        let mut place = self.cursor.to_place();
        place.projection.push(PlaceElem::Deref);
        self.insertions.push((
            loc,
            place,
            Binding { kind: LocKind::Box(alloc.clone()), ty: deref_ty.clone() },
        ));
        loc
    }

    fn field(&mut self, ty: &Ty, f: FieldIdx) -> CheckerResult<Ty> {
        let ty = match ty.kind() {
            TyKind::Indexed(BaseTy::Tuple(fields), idx) => {
                let mut fields = fields.to_vec();
                fields[f.as_usize()] = fields[f.as_usize()].try_fold_with(self)?;
                Ty::indexed(BaseTy::Tuple(fields.into()), idx.clone())
            }
            TyKind::Indexed(BaseTy::Closure(def_id, upvar_tys, args), idx) => {
                let mut upvar_tys = upvar_tys.to_vec();
                upvar_tys[f.as_usize()] = upvar_tys[f.as_usize()].try_fold_with(self)?;
                Ty::indexed(BaseTy::Closure(*def_id, upvar_tys.into(), args.clone()), idx.clone())
            }
            TyKind::Indexed(BaseTy::Coroutine(def_id, resume_ty, upvar_tys), idx) => {
                let mut upvar_tys = upvar_tys.to_vec();
                upvar_tys[f.as_usize()] = upvar_tys[f.as_usize()].try_fold_with(self)?;
                Ty::indexed(
                    BaseTy::Coroutine(*def_id, resume_ty.clone(), upvar_tys.into()),
                    idx.clone(),
                )
            }
            TyKind::Indexed(BaseTy::Adt(adt, args), idx) => {
                let mut fields = downcast_struct(self.infcx, adt, args, idx)?
                    .into_iter()
                    .map(|ty| self.unpack_for_downcast(&ty))
                    .collect_vec();
                fields[f.as_usize()] = fields[f.as_usize()].try_fold_with(self)?;
                let args = args.with_holes();
                Ty::downcast(adt.clone(), args, ty.clone(), FIRST_VARIANT, fields.into())
            }
            TyKind::Downcast(adt, args, ty, variant, fields) => {
                let mut fields = fields.to_vec();
                fields[f.as_usize()] = fields[f.as_usize()].try_fold_with(self)?;
                Ty::downcast(adt.clone(), args.clone(), ty.clone(), *variant, fields.into())
            }
            _ => tracked_span_bug!("invalid field access for `{ty:?}`"),
        };
        Ok(ty)
    }

    fn downcast(&mut self, ty: &Ty, variant: VariantIdx) -> CheckerResult<Ty> {
        let ty = match ty.kind() {
            TyKind::Indexed(BaseTy::Adt(adt, args), idx) => {
                let fields = downcast(self.infcx, adt, args, variant, idx)?
                    .into_iter()
                    .map(|ty| self.unpack_for_downcast(&ty))
                    .collect_vec();
                Ty::downcast(adt.clone(), args.with_holes(), ty.clone(), variant, fields.into())
            }
            TyKind::Downcast(.., variant2, _) => {
                debug_assert_eq!(variant, *variant2);
                ty.clone()
            }
            _ => tracked_span_bug!("invalid downcast `{ty:?}`"),
        };
        Ok(ty)
    }

    fn index(&mut self, ty: &Ty) -> CheckerResult {
        match ty.kind() {
            TyKind::Indexed(BaseTy::Array(arr_ty, _), _) => {
                arr_ty.try_fold_with(self)?;
            }
            TyKind::Indexed(BaseTy::Slice(slice_ty), _) => {
                slice_ty.try_fold_with(self)?;
            }
            _ => tracked_span_bug!("invalid index on `{ty:?}`"),
        }
        Ok(())
    }

    fn unpack(&mut self, ty: &Ty) -> Ty {
        self.infcx
            .hoister(AssumeInvariants::yes(self.checker_conf.check_overflow))
            .shallow()
            .hoist(ty)
    }

    fn unpack_for_downcast(&mut self, ty: &Ty) -> Ty {
        let ty = self
            .infcx
            .hoister(AssumeInvariants::No)
            .hoist_existentials(self.in_ref != Some(Mutability::Mut))
            .hoist(ty);
        self.infcx
            .assume_invariants(&ty, self.checker_conf.check_overflow);
        ty
    }

    fn change_root(&mut self, path: &Path) {
        self.has_work = true;
        self.cursor.change_root(path);
    }

    fn should_continue(&mut self) -> bool {
        if self.has_work {
            self.has_work = false;
            true
        } else {
            false
        }
    }
}

struct Updater<F> {
    new_ty: F,
    cursor: Cursor,
}

impl<F> Updater<F>
where
    F: FnOnce(Cursor, &Ty) -> Ty,
{
    fn new(cursor: Cursor, new_ty: F) -> Self {
        Self { new_ty, cursor }
    }

    fn update(bindings: &mut PlacesTree, cursor: Cursor, new_ty: F) {
        let binding = bindings.get_loc_mut(&cursor.loc);
        let lookup = Updater::new(cursor, new_ty);
        binding.ty = lookup.fold_ty(&binding.ty);
    }

    fn fold_ty(mut self, ty: &Ty) -> Ty {
        let Some(elem) = self.cursor.next() else {
            return (self.new_ty)(self.cursor, ty);
        };
        match elem {
            PlaceElem::Deref => self.deref(ty),
            PlaceElem::Field(f) => self.field(ty, f),
            PlaceElem::Downcast(_, _) => self.fold_ty(ty),
            PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                // When unblocking under an array/slice we stop at the array/slice because the entire
                // array has to be blocked when taking references to an element
                (self.new_ty)(self.cursor, ty)
            }
        }
    }

    fn deref(self, ty: &Ty) -> Ty {
        match ty.kind() {
            TyKind::Indexed(BaseTy::Adt(adt, args), idx) if adt.is_box() => {
                let (deref_ty, alloc_ty) = args.box_args();
                let args = List::from_arr([
                    GenericArg::Ty(self.fold_ty(deref_ty)),
                    GenericArg::Ty(alloc_ty.clone()),
                ]);
                Ty::indexed(BaseTy::Adt(adt.clone(), args), idx.clone())
            }
            TyKind::Ptr(..) => {
                tracked_span_bug!("cannot update through pointer");
            }
            Ref!(re, deref_ty, mutbl) => Ty::mk_ref(*re, self.fold_ty(deref_ty), *mutbl),
            _ => tracked_span_bug!("invalid deref on `{ty:?}`"),
        }
    }

    fn field(self, ty: &Ty, f: FieldIdx) -> Ty {
        match ty.kind() {
            TyKind::Indexed(BaseTy::Tuple(fields), idx) => {
                let fields = self.fold_field_at(fields, f);
                Ty::indexed(BaseTy::Tuple(fields), idx.clone())
            }
            TyKind::Indexed(BaseTy::Closure(def_id, upvar_tys, args), idx) => {
                let upvar_tys = self.fold_field_at(upvar_tys, f);
                Ty::indexed(BaseTy::Closure(*def_id, upvar_tys, args.clone()), idx.clone())
            }
            TyKind::Indexed(BaseTy::Coroutine(def_id, resume_ty, upvar_tys), idx) => {
                let upvar_tys = self.fold_field_at(upvar_tys, f);
                Ty::indexed(BaseTy::Coroutine(*def_id, resume_ty.clone(), upvar_tys), idx.clone())
            }
            TyKind::Downcast(adt, args, ty, variant, fields) => {
                let fields = self.fold_field_at(fields, f);
                Ty::downcast(adt.clone(), args.clone(), ty.clone(), *variant, fields)
            }
            _ => tracked_span_bug!("invalid field projection on `{ty:?}`"),
        }
    }

    fn fold_field_at(self, fields: &[Ty], f: FieldIdx) -> List<Ty> {
        let mut fields = fields.to_vec();
        fields[f.as_usize()] = self.fold_ty(&fields[f.as_usize()]);
        fields.into()
    }
}

struct Cursor {
    loc: Loc,
    place: Place,
    proj: Vec<PlaceElem>,
    pos: usize,
}

impl Cursor {
    fn new(key: &impl LookupKey, place: Place) -> Self {
        let proj = key.proj().rev().collect_vec();
        let pos = proj.len();
        Self { loc: key.loc(), place, proj, pos }
    }

    fn change_root(&mut self, path: &Path) {
        self.place = self.to_place();
        self.proj.truncate(self.pos);
        self.proj.extend(
            path.projection()
                .iter()
                .rev()
                .copied()
                .map(PlaceElem::Field),
        );
        self.loc = path.loc;
        self.pos = self.proj.len();
    }

    fn to_place(&self) -> Place {
        let mut place = self.place.clone();
        place
            .projection
            .extend(self.proj.iter().skip(self.pos).rev().copied());
        place
    }

    fn to_path(&self) -> Path {
        let mut proj = vec![];
        for elem in self.proj.iter().rev() {
            match *elem {
                PlaceElem::Field(f) => proj.push(f),
                PlaceElem::Downcast(_, _) => {}
                PlaceElem::Deref | PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                    break;
                }
            }
        }
        Path::new(self.loc, List::from_vec(proj))
    }

    fn next(&mut self) -> Option<PlaceElem> {
        if self.pos > 0 {
            self.pos -= 1;
            Some(self.proj[self.pos])
        } else {
            None
        }
    }

    fn is_exhausted(&self) -> bool {
        self.pos == 0
    }

    fn reset(&mut self) {
        self.pos = self.proj.len();
    }
}

impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{loc: {:?}, place: {:?}, proj: [", self.loc, self.place)?;
        for (i, elem) in self.proj.iter().enumerate().rev() {
            if i < self.proj.len() - 1 {
                write!(f, ", ")?;
            }
            if i + 1 == self.pos {
                write!(f, ">")?;
            }
            write!(f, "{elem:?}")?;
        }
        write!(f, "]}}")
    }
}

fn downcast(
    infcx: &mut InferCtxt,
    adt: &AdtDef,
    args: &[GenericArg],
    variant_idx: VariantIdx,
    idx: &Expr,
) -> CheckerResult<Vec<Ty>> {
    if adt.is_struct() {
        debug_assert_eq!(variant_idx.as_u32(), 0);
        downcast_struct(infcx, adt, args, idx)
    } else if adt.is_enum() {
        downcast_enum(infcx, adt, variant_idx, args, idx)
    } else {
        tracked_span_bug!("Downcast without struct or enum!")
    }
}

/// `downcast` on struct works as follows
/// Given a struct definition
///     struct S<A..>[(i...)] { fld : T, ...}
/// and a
///     * "place" `x: S<t..>[e..]`
/// the `downcast` returns a vector of `ty` for each `fld` of `x` where
///     * `x.fld : T[A := t ..][i := e...]`
/// i.e. by substituting the type and value indices using the types and values from `x`.
fn downcast_struct(
    infcx: &InferCtxt,
    adt: &AdtDef,
    args: &[GenericArg],
    idx: &Expr,
) -> CheckerResult<Vec<Ty>> {
    let tcx = infcx.genv.tcx();
    let flds = adt
        .sort_def()
        .projections()
        .map(|proj| idx.proj_and_reduce(proj))
        .collect_vec();
    Ok(struct_variant(infcx.genv, adt.did())?
        .instantiate(tcx, args, &[])
        .replace_bound_refts(&flds)
        .normalize_projections(infcx.genv, infcx.region_infcx, infcx.def_id)?
        .fields
        .to_vec())
}

fn struct_variant(
    genv: GlobalEnv,
    def_id: DefId,
) -> CheckerResult<EarlyBinder<Binder<VariantSig>>> {
    let adt_def = genv.adt_def(def_id)?;
    debug_assert!(adt_def.is_struct() || adt_def.is_union());
    genv.variant_sig(def_id, VariantIdx::from_u32(0))?
        .ok_or_else(|| CheckerErrKind::OpaqueStruct(def_id))
}

/// In contrast (w.r.t. `struct`) downcast on `enum` works as follows.
/// Given
///     * a "place" `x : T[i..]`
///     * a "variant" of type `forall z..,(y:t...) => E[j...]`
/// We want `downcast` to return a vector of types _and an assertion_ by
///     1. *Instantiate* the type to fresh names `z'...` to get `(y:t'...) => T[j'...]`
///     2. *Unpack* the fields using `y:t'...`
///     3. *Assert* the constraint `i == j'...`
fn downcast_enum(
    infcx: &mut InferCtxt,
    adt: &AdtDef,
    variant_idx: VariantIdx,
    args: &[GenericArg],
    idx1: &Expr,
) -> CheckerResult<Vec<Ty>> {
    let tcx = infcx.genv.tcx();
    let variant_def = infcx
        .genv
        .variant_sig(adt.did(), variant_idx)?
        .expect("enums cannot be opaque")
        .instantiate(tcx, args, &[])
        .replace_bound_refts_with(|sort, _, _| infcx.define_vars(sort))
        .normalize_projections(infcx.genv, infcx.region_infcx, infcx.def_id)?;

    // FIXME(nilehmann) We could assert idx1 == variant_def.idx directly, but for aggregate sorts there
    // are currently two problems.
    // 1. The encoded fixpoint constraint won't parse if it has nested expressions inside data constructors.
    // 2. We could expand the equality during encoding, but that would require annotating the sort
    // of the equality operator, which will be cumbersome because we create equalities in some places where
    // the sort is not readily available.
    let constr = Expr::and_from_iter(adt.sort_def().projections().map(|proj| {
        let e1 = idx1.proj_and_reduce(proj);
        let e2 = variant_def.idx.proj_and_reduce(proj);
        Expr::eq(e1, e2)
    }));
    infcx.assume_pred(&constr);

    Ok(variant_def.fields.to_vec())
}

fn fold(
    bindings: &mut PlacesTree,
    infcx: &mut InferCtxtAt,
    ty: &Ty,
    is_strg: bool,
) -> CheckerResult<Ty> {
    match ty.kind() {
        TyKind::Ptr(PtrKind::Box, path) => {
            let loc = path.to_loc().unwrap_or_else(|| tracked_span_bug!());
            let binding = bindings.remove(&loc);
            let LocKind::Box(alloc) = binding.kind else {
                tracked_span_bug!("box pointer to non-box loc");
            };
            let deref_ty = fold(bindings, infcx, &binding.ty, is_strg)?;
            Ok(Ty::mk_box(infcx.genv, deref_ty, alloc)?)
        }
        Ref!(re, deref_ty, mutbl) => {
            let deref_ty = fold(bindings, infcx, deref_ty, is_strg)?;
            Ok(Ty::mk_ref(*re, deref_ty, *mutbl))
        }
        TyKind::Downcast(adt, args, ty_, variant_idx, fields) => {
            if is_strg {
                let variant_sig = infcx
                    .genv
                    .variant_sig(adt.did(), *variant_idx)?
                    .expect("unexpected opaque struct");

                let fields = fields
                    .iter()
                    .map(|ty| fold(bindings, infcx, ty, is_strg))
                    .try_collect_vec()?;

                let partially_moved = fields.iter().any(Ty::is_uninit);
                let ty = if partially_moved {
                    Ty::uninit()
                } else {
                    infcx
                        .check_constructor(variant_sig, args, &fields, ConstrReason::Fold)
                        .unwrap_or_else(|err| tracked_span_bug!("{err:?}"))
                };

                Ok(ty)
            } else {
                Ok(ty_.clone())
            }
        }
        TyKind::Indexed(BaseTy::Tuple(fields), idx) => {
            debug_assert_eq!(idx, &Expr::unit());

            let fields = fields
                .iter()
                .map(|ty| fold(bindings, infcx, ty, is_strg))
                .try_collect_vec()?;

            let partially_moved = fields.iter().any(Ty::is_uninit);
            let ty = if partially_moved { Ty::uninit() } else { Ty::tuple(fields) };
            Ok(ty)
        }
        _ => Ok(ty.clone()),
    }
}

mod pretty {

    use flux_middle::pretty::*;
    use rustc_middle::ty::TyCtxt;

    use super::*;

    impl Pretty for PlacesTree {
        fn fmt(&self, cx: &PrettyCx, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            define_scoped!(cx, f);
            w!(
                "{{{}}}",
                ^self
                    .iter()
                    .filter(|(_, binding)| !cx.hide_uninit || !binding.ty.is_uninit())
                    .sorted_by(|(loc1, _), (loc2, _)| loc1.cmp(loc2))
                    .format_with(", ", |(loc, binding), f| {
                        f(&format_args_cx!("{:?}:{:?} {:?}", loc, &binding.kind, &binding.ty))
                    })
            )
        }

        fn default_cx(tcx: TyCtxt) -> PrettyCx {
            PrettyCx::default(tcx)
                .kvar_args(KVarArgs::Hide)
                .hide_binder(true)
        }
    }

    impl Pretty for LocKind {
        fn fmt(&self, _cx: &PrettyCx, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            define_scoped!(cx, f);
            match self {
                LocKind::Local | LocKind::Universal => Ok(()),
                LocKind::Box(_) => w!("[box]"),
            }
        }
    }

    impl_debug_with_default_cx!(PlacesTree);
}

impl TypeVisitable for PlacesTree {
    fn visit_with<V: TypeVisitor>(&self, _visitor: &mut V) -> ControlFlow<V::BreakTy> {
        unimplemented!()
    }
}

impl TypeFoldable for PlacesTree {
    fn try_fold_with<F: FallibleTypeFolder>(&self, folder: &mut F) -> Result<Self, F::Error> {
        let mut this = self.clone();
        this.try_fmap_mut(|ty| ty.try_fold_with(folder))?;
        Ok(this)
    }
}
