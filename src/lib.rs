use quote::quote;
use syn::visit_mut::VisitMut;
use std::collections::HashMap;

#[derive(Default)]
struct Clown {
	honks: HashMap<syn::Ident, syn::Expr>,
}

impl syn::visit_mut::VisitMut for Clown {
	fn visit_expr_mut(&mut self, this_expr: &mut syn::Expr) {
		match this_expr {
			syn::Expr::Macro(syn::ExprMacro { mac, .. }) => {
				if !mac.path.get_ident().map_or(false, |x| x == "honk") { return syn::visit_mut::visit_expr_mut(self, this_expr); }
				let expr = syn::parse2::<syn::Expr>(mac.tokens.clone()).expect("honk! macro argument must be an expression");
				let ident = quote::format_ident!("__honk_{}", self.honks.len());
				*this_expr = syn::parse_quote!(#ident);
				self.honks.insert(ident, expr);
			},
			// do not recurse into closures in case there's a nested clown
			syn::Expr::Closure(_) => {},
			_ => syn::visit_mut::visit_expr_mut(self, this_expr),
		}
	}
}

/// expands `#[clown] || do_call(honk!(foo.bar))` to `{ let __honk_0 = ::core::clone::Clone::clone(&foo.bar); move || do_call(__honk_0) }` as some approximation of "capture-by-clone" closure
#[proc_macro_attribute]
pub fn clown(_: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut item = syn::parse_macro_input!(item as syn::ExprClosure);
	item.capture = Some(syn::parse_quote!(move));
	let mut clown = Clown::default();
	clown.visit_expr_closure_mut(&mut item);

	let honks = clown.honks.into_iter().map(|(ident, expr)| quote! { let #ident = ::core::clone::Clone::clone(&#expr);});
	quote! {
		{
			#(#honks)*
			#item
		}
	}.into()
}
