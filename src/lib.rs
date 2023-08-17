use quote::{quote, quote_spanned};
use syn::{visit_mut::VisitMut, spanned::Spanned};
use std::collections::HashMap;
use proc_macro2::{TokenStream, TokenTree};
use itertools::Itertools;

#[derive(Default)]
struct Clown {
	honks: HashMap<syn::Ident, syn::Expr>,
}

impl Clown {
	fn raw_tt_visit(&mut self, tt: TokenStream, acc: &mut TokenStream) {
		let mut tt_iter = tt.into_iter().multipeek();
		while let Some(tree) = tt_iter.next() {
			use std::iter::once;

			match tree {
				TokenTree::Group(group) => {
					let mut acc_rec = TokenStream::new();
					self.raw_tt_visit(group.stream(), &mut acc_rec);
					acc.extend(once(TokenTree::Group(proc_macro2::Group::new(group.delimiter(), acc_rec))));
				},
				TokenTree::Literal(_) | TokenTree::Punct(_) => acc.extend(once(tree)),
				TokenTree::Ident(ref ident) => {
					if ident != "honk" { acc.extend(once(tree)); continue; }
					let Some(TokenTree::Punct(punct)) = tt_iter.peek() else { acc.extend(once(tree)); continue; };
					if punct.as_char() != '!' { acc.extend(once(tree)); continue; }
					let Some(TokenTree::Group(group)) = tt_iter.peek() else { acc.extend(once(tree)); continue; };
					let expr = syn::parse2::<syn::Expr>(group.stream()).expect("honk! macro argument must be an expression");
					let ident = syn::Ident::new(&format!("__honk_{}", self.honks.len()), expr.span());
					acc.extend(once(TokenTree::Ident(ident.clone())));
					self.honks.insert(ident, expr);

					// we got `honk` as next, now skip `!` and group with args
					tt_iter.next(); tt_iter.next();
				},
			}
		}
	}
}

impl syn::visit_mut::VisitMut for Clown {
	fn visit_macro_mut(&mut self, this_macro: &mut syn::Macro) {
		let mut acc = TokenStream::new();
		self.raw_tt_visit(this_macro.tokens.clone(), &mut acc);
		this_macro.tokens = acc;
	}

	fn visit_expr_mut(&mut self, this_expr: &mut syn::Expr) {
		match this_expr {
			syn::Expr::Macro(syn::ExprMacro { mac, .. }) => {
				if !mac.path.get_ident().is_some_and(|x| x == "honk") { return syn::visit_mut::visit_expr_mut(self, this_expr); }
				let expr = syn::parse2::<syn::Expr>(mac.tokens.clone()).expect("honk! macro argument must be an expression");
				let ident = syn::Ident::new(&format!("__honk_{}", self.honks.len()), this_expr.span());
				*this_expr = syn::parse_quote!(#ident);
				self.honks.insert(ident, expr);
			},
			// do not recurse into closures in case there's a nested clown
			syn::Expr::Closure(_) => {},
			_ => syn::visit_mut::visit_expr_mut(self, this_expr),
		}
	}
}

/// expands `#[clown] || do_call(honk!(foo.bar))` to `{ let __honk_0 = (foo.bar).clone(); move || do_call(__honk_0) }` as some approximation of "capture-by-clone" closure
#[proc_macro_attribute]
pub fn clown(_: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut item = syn::parse_macro_input!(item as syn::ExprClosure);
	item.capture = Some(syn::parse_quote!(move));
	let mut clown = Clown::default();
	clown.visit_expr_closure_mut(&mut item);

	let honks = clown.honks.into_iter().map(|(ident, expr)| { let span = expr.span(); quote_spanned! {span=> let #ident = (#expr).clone();} });

	quote! {
		{
			#(#honks)*
			#item
		}
	}.into()
}
