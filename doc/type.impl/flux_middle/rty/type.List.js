(function() {var type_impls = {
"flux_middle":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-TypeFoldable-for-Interned%3C%5BT%5D%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#935-943\">source</a><a href=\"#impl-TypeFoldable-for-Interned%3C%5BT%5D%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T&gt; <a class=\"trait\" href=\"flux_middle/rty/fold/trait.TypeFoldable.html\" title=\"trait flux_middle::rty::fold::TypeFoldable\">TypeFoldable</a> for <a class=\"type\" href=\"flux_middle/rty/type.List.html\" title=\"type flux_middle::rty::List\">List</a>&lt;T&gt;<div class=\"where\">where\n    T: <a class=\"trait\" href=\"flux_middle/rty/fold/trait.TypeFoldable.html\" title=\"trait flux_middle::rty::fold::TypeFoldable\">TypeFoldable</a>,\n    <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.slice.html\">[T]</a>: <a class=\"trait\" href=\"flux_arc_interner/trait.Internable.html\" title=\"trait flux_arc_interner::Internable\">Internable</a>,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.try_fold_with\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#940-942\">source</a><a href=\"#method.try_fold_with\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#tymethod.try_fold_with\" class=\"fn\">try_fold_with</a>&lt;F: <a class=\"trait\" href=\"flux_middle/rty/fold/trait.FallibleTypeFolder.html\" title=\"trait flux_middle::rty::fold::FallibleTypeFolder\">FallibleTypeFolder</a>&gt;(\n    &amp;self,\n    folder: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.reference.html\">&amp;mut F</a>,\n) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;Self, F::<a class=\"associatedtype\" href=\"flux_middle/rty/fold/trait.FallibleTypeFolder.html#associatedtype.Error\" title=\"type flux_middle::rty::fold::FallibleTypeFolder::Error\">Error</a>&gt;</h4></section><section id=\"method.fold_with\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#230-232\">source</a><a href=\"#method.fold_with\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.fold_with\" class=\"fn\">fold_with</a>&lt;F: <a class=\"trait\" href=\"flux_middle/rty/fold/trait.TypeFolder.html\" title=\"trait flux_middle::rty::fold::TypeFolder\">TypeFolder</a>&gt;(&amp;self, folder: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.reference.html\">&amp;mut F</a>) -&gt; Self</h4></section><section id=\"method.normalize_projections\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#234-242\">source</a><a href=\"#method.normalize_projections\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.normalize_projections\" class=\"fn\">normalize_projections</a>&lt;'tcx&gt;(\n    &amp;self,\n    genv: <a class=\"struct\" href=\"flux_middle/global_env/struct.GlobalEnv.html\" title=\"struct flux_middle::global_env::GlobalEnv\">GlobalEnv</a>&lt;'_, 'tcx&gt;,\n    infcx: &amp;<a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/nightly-rustc/rustc_infer/infer/struct.InferCtxt.html\" title=\"struct rustc_infer::infer::InferCtxt\">InferCtxt</a>&lt;'tcx&gt;,\n    callsite_def_id: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/nightly-rustc/rustc_span/def_id/struct.DefId.html\" title=\"struct rustc_span::def_id::DefId\">DefId</a>,\n) -&gt; <a class=\"type\" href=\"flux_middle/queries/type.QueryResult.html\" title=\"type flux_middle::queries::QueryResult\">QueryResult</a>&lt;Self&gt;</h4></section><details class=\"toggle method-toggle\" open><summary><section id=\"method.normalize\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#245-247\">source</a><a href=\"#method.normalize\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.normalize\" class=\"fn\">normalize</a>(&amp;self, defns: &amp;<a class=\"struct\" href=\"flux_middle/rty/normalize/struct.SpecFuncDefns.html\" title=\"struct flux_middle::rty::normalize::SpecFuncDefns\">SpecFuncDefns</a>) -&gt; Self</h4></section></summary><div class='docblock'>Normalize expressions by applying beta reductions for tuples and lambda abstractions.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.replace_holes\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#256-280\">source</a><a href=\"#method.replace_holes\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.replace_holes\" class=\"fn\">replace_holes</a>(&amp;self, f: impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/function/trait.FnMut.html\" title=\"trait core::ops::function::FnMut\">FnMut</a>(&amp;[<a class=\"type\" href=\"flux_middle/rty/type.List.html\" title=\"type flux_middle::rty::List\">List</a>&lt;<a class=\"enum\" href=\"flux_middle/rty/enum.Sort.html\" title=\"enum flux_middle::rty::Sort\">Sort</a>&gt;], <a class=\"enum\" href=\"flux_middle/rty/expr/enum.HoleKind.html\" title=\"enum flux_middle::rty::expr::HoleKind\">HoleKind</a>) -&gt; <a class=\"struct\" href=\"flux_middle/rty/expr/struct.Expr.html\" title=\"struct flux_middle::rty::expr::Expr\">Expr</a>) -&gt; Self</h4></section></summary><div class='docblock'>Replaces all <a href=\"flux_middle/rty/expr/enum.ExprKind.html#variant.Hole\" title=\"variant flux_middle::rty::expr::ExprKind::Hole\">holes</a> with the result of calling a closure. The closure takes a list with\nall the <em>layers</em> of <a href=\"flux_middle/rty/struct.Binder.html\" title=\"struct flux_middle::rty::Binder\">bound</a> variables at the point the hole was found. Each layer corresponds\nto the list of sorts bound at that level. The list is ordered from outermost to innermost\nbinder, i.e., the last element is the binder closest to the hole.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.with_holes\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#287-301\">source</a><a href=\"#method.with_holes\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.with_holes\" class=\"fn\">with_holes</a>(&amp;self) -&gt; Self</h4></section></summary><div class='docblock'>Remove all refinements and turn each underlying <a href=\"flux_middle/rty/enum.BaseTy.html\" title=\"enum flux_middle::rty::BaseTy\"><code>BaseTy</code></a> into a <a href=\"flux_middle/rty/enum.TyKind.html#variant.Exists\" title=\"variant flux_middle::rty::TyKind::Exists\"><code>TyKind::Exists</code></a> with a\n<a href=\"flux_middle/rty/enum.TyKind.html#variant.Constr\" title=\"variant flux_middle::rty::TyKind::Constr\"><code>TyKind::Constr</code></a> and a <a href=\"flux_middle/rty/expr/enum.ExprKind.html#variant.Hole\" title=\"variant flux_middle::rty::expr::ExprKind::Hole\"><code>hole</code></a>. For example, <code>Vec&lt;{v. i32[v] | v &gt; 0}&gt;[n]</code> becomes\n<code>{n. Vec&lt;{v. i32[v] | *}&gt;[n] | *}</code>.</div></details><section id=\"method.replace_evars\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#303-306\">source</a><a href=\"#method.replace_evars\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.replace_evars\" class=\"fn\">replace_evars</a>(&amp;self, evars: &amp;<a class=\"struct\" href=\"flux_middle/rty/evars/struct.EVarSol.html\" title=\"struct flux_middle::rty::evars::EVarSol\">EVarSol</a>) -&gt; Self</h4></section><section id=\"method.shift_in_escaping\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#308-346\">source</a><a href=\"#method.shift_in_escaping\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.shift_in_escaping\" class=\"fn\">shift_in_escaping</a>(&amp;self, amount: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>) -&gt; Self</h4></section><section id=\"method.shift_out_escaping\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#348-383\">source</a><a href=\"#method.shift_out_escaping\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.shift_out_escaping\" class=\"fn\">shift_out_escaping</a>(&amp;self, amount: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>) -&gt; Self</h4></section><section id=\"method.erase_regions\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#385-397\">source</a><a href=\"#method.erase_regions\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeFoldable.html#method.erase_regions\" class=\"fn\">erase_regions</a>(&amp;self) -&gt; Self</h4></section></div></details>","TypeFoldable","flux_middle::rty::Clauses","flux_middle::rty::PolyVariants","flux_middle::rty::RefineArgs","flux_middle::rty::GenericArgs"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-TypeVisitable-for-Interned%3C%5BT%5D%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#925-933\">source</a><a href=\"#impl-TypeVisitable-for-Interned%3C%5BT%5D%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T&gt; <a class=\"trait\" href=\"flux_middle/rty/fold/trait.TypeVisitable.html\" title=\"trait flux_middle::rty::fold::TypeVisitable\">TypeVisitable</a> for <a class=\"type\" href=\"flux_middle/rty/type.List.html\" title=\"type flux_middle::rty::List\">List</a>&lt;T&gt;<div class=\"where\">where\n    T: <a class=\"trait\" href=\"flux_middle/rty/fold/trait.TypeVisitable.html\" title=\"trait flux_middle::rty::fold::TypeVisitable\">TypeVisitable</a>,\n    <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.slice.html\">[T]</a>: <a class=\"trait\" href=\"flux_arc_interner/trait.Internable.html\" title=\"trait flux_arc_interner::Internable\">Internable</a>,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.visit_with\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#930-932\">source</a><a href=\"#method.visit_with\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeVisitable.html#tymethod.visit_with\" class=\"fn\">visit_with</a>&lt;V: <a class=\"trait\" href=\"flux_middle/rty/fold/trait.TypeVisitor.html\" title=\"trait flux_middle::rty::fold::TypeVisitor\">TypeVisitor</a>&gt;(&amp;self, visitor: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.reference.html\">&amp;mut V</a>) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/ops/control_flow/enum.ControlFlow.html\" title=\"enum core::ops::control_flow::ControlFlow\">ControlFlow</a>&lt;V::<a class=\"associatedtype\" href=\"flux_middle/rty/fold/trait.TypeVisitor.html#associatedtype.BreakTy\" title=\"type flux_middle::rty::fold::TypeVisitor::BreakTy\">BreakTy</a>&gt;</h4></section><section id=\"method.has_escaping_bvars\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#169-201\">source</a><a href=\"#method.has_escaping_bvars\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeVisitable.html#method.has_escaping_bvars\" class=\"fn\">has_escaping_bvars</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.bool.html\">bool</a></h4></section><details class=\"toggle method-toggle\" open><summary><section id=\"method.fvars\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/flux_middle/rty/fold.rs.html#205-220\">source</a><a href=\"#method.fvars\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"flux_middle/rty/fold/trait.TypeVisitable.html#method.fvars\" class=\"fn\">fvars</a>(&amp;self) -&gt; FxHashSet&lt;<a class=\"struct\" href=\"flux_middle/rty/expr/struct.Name.html\" title=\"struct flux_middle::rty::expr::Name\">Name</a>&gt;</h4></section></summary><div class='docblock'>Returns the set of all free variables.\nFor example, <code>Vec&lt;i32[n]&gt;{v : v &gt; m}</code> returns <code>{n, m}</code>.</div></details></div></details>","TypeVisitable","flux_middle::rty::Clauses","flux_middle::rty::PolyVariants","flux_middle::rty::RefineArgs","flux_middle::rty::GenericArgs"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()