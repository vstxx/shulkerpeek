// Source audit: `audit ORIGINAL STAGE PLAN`. Checks that each planned HEAD
// statement (plain or cancellable) is exactly the planned helper and
// bindings, and each support module exactly its planned source, then removes
// only those and compares every affected file's AST with the pristine source.
// Adapted from horizon-native-mod-template (MIT).
use quote::quote;
use std::{collections::BTreeMap, env, fs, path::Path};
use syn::{Block, Item};

fn block<'a>(items: &'a mut [Item], node: &[usize]) -> &'a mut Block {
    let (index, rest) = node.split_first().unwrap();
    match &mut items[*index] {
        Item::Fn(f) if rest.is_empty() => &mut f.block,
        Item::Impl(i) if rest.len() == 1 => match &mut i.items[rest[0]] {
            syn::ImplItem::Fn(f) => &mut f.block,
            _ => panic!("planned method absent"),
        },
        Item::Mod(m) => block(&mut m.content.as_mut().unwrap().1, rest),
        _ => panic!("planned target absent"),
    }
}

/// Undoes the one difference the transformer's printing makes: prettyplease
/// writes a closure body that would not fit on its line as `{ body }`, which
/// parses as a block. The same closure, with or without the braces, compares
/// equal.
struct ClosureBodies;

impl syn::visit_mut::VisitMut for ClosureBodies {
    fn visit_expr_closure_mut(&mut self, closure: &mut syn::ExprClosure) {
        syn::visit_mut::visit_expr_closure_mut(self, closure);
        if let syn::Expr::Block(b) = &*closure.body
            && b.attrs.is_empty()
            && b.label.is_none()
            && let [syn::Stmt::Expr(inner, None)] = &b.block.stmts[..]
        {
            let inner = inner.clone();
            *closure.body = inner;
        }
    }
}

fn canonical(items: &[Item]) -> String {
    let mut file = syn::File {
        shebang: None,
        attrs: vec![],
        items: items.to_vec(),
    };
    syn::visit_mut::VisitMut::visit_file_mut(&mut ClosureBodies, &mut file);
    prettyplease::unparse(&file)
}

/// The statement horizon-mixins generates for a HEAD injection.
fn expected_head(injection: &serde_json::Value) -> syn::Stmt {
    assert_eq!(
        injection["kind"], "HEAD",
        "only HEAD injections are audited"
    );
    let helper: syn::ItemFn = syn::parse_str(injection["helper_source"].as_str().unwrap()).unwrap();
    let name = &helper.sig.ident;
    let args: Vec<syn::Expr> = injection["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| syn::parse_str(b["value"].as_str().unwrap()).unwrap())
        .collect();
    let tokens = if injection["cancellable"] == true {
        quote!(if let ::core::ops::ControlFlow::Break(__horizon_mixins_return) = {
            #helper #name(#(#args),*)
        } {
            return __horizon_mixins_return;
        })
    } else {
        quote!({#helper #name(#(#args),*)};)
    };
    syn::parse2(tokens).unwrap()
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let plan: serde_json::Value = serde_json::from_slice(&fs::read(&args[3]).unwrap()).unwrap();
    let mut trees = BTreeMap::new();
    // The plan lists same-function hooks in the order they run, which is the
    // order of their statements: each is the body's first when it is reached.
    for p in plan["injections"].as_array().unwrap() {
        let file = p["file"].as_str().unwrap();
        let tree = trees.entry(file).or_insert_with(|| {
            syn::parse_file(&fs::read_to_string(Path::new(&args[2]).join(file)).unwrap()).unwrap()
        });
        let node: Vec<usize> = p["node"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_u64().unwrap() as usize)
            .collect();
        let expected = expected_head(p);
        let b = block(&mut tree.items, &node);
        let actual = b.stmts.first().expect("HEAD missing");
        let actual_fn: Item = syn::parse_quote!(fn audit() {#actual});
        let expected_fn: Item = syn::parse_quote!(fn audit() {#expected});
        assert_eq!(
            canonical(&[actual_fn]),
            canonical(&[expected_fn]),
            "HEAD differs from planned helper/bindings: {}",
            p["injection_id"]
        );
        b.stmts.remove(0);
    }
    for m in plan["modules"].as_array().unwrap() {
        let file = m["file"].as_str().unwrap();
        let tree = trees.entry(file).or_insert_with(|| {
            syn::parse_file(&fs::read_to_string(Path::new(&args[2]).join(file)).unwrap()).unwrap()
        });
        let index = tree
            .items
            .iter()
            .position(|i| matches!(i, Item::Mod(v) if v.ident == m["name"].as_str().unwrap()))
            .expect("support module missing");
        let Item::Mod(module) = tree.items.remove(index) else {
            unreachable!()
        };
        let payload = syn::parse_file(m["source"].as_str().unwrap()).unwrap();
        assert!(module.attrs.is_empty());
        // The plan names `pub`; it omits the default, `pub(crate)`.
        if m["visibility"] == "pub" {
            assert!(matches!(&module.vis, syn::Visibility::Public(_)));
        } else {
            assert!(
                matches!(&module.vis, syn::Visibility::Restricted(v) if v.path.is_ident("crate"))
            );
        }
        let mut expected = module.clone();
        expected.content.as_mut().unwrap().1 = payload.items;
        assert!(
            canonical(&[Item::Mod(module)]) == canonical(&[Item::Mod(expected)]),
            "support module differs from planned source"
        );
    }
    for (file, tree) in trees {
        let original =
            syn::parse_file(&fs::read_to_string(Path::new(&args[1]).join(file)).unwrap()).unwrap();
        assert!(
            canonical(&original.items) == canonical(&tree.items),
            "unexpected source change in {file}"
        );
    }
    println!(
        "all original ASTs preserved apart from the planned HEAD statements and support module"
    );
}
