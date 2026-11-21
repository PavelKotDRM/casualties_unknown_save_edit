use std::collections::BTreeSet;
#[cfg(not(test))]
use std::{error::Error, fs, path::Path};

use syn::parse::Parser;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprCall, ExprMatch, ExprMethodCall, ExprStruct, ItemFn, ItemMod, Lit};

#[derive(Default)]
struct Strings(BTreeSet<String>);

impl Strings {
    fn expression(&mut self, expression: &Expr) {
        struct Literals<'a>(&'a mut BTreeSet<String>);
        impl<'ast> Visit<'ast> for Literals<'_> {
            fn visit_lit(&mut self, literal: &'ast Lit) {
                if let Lit::Str(text) = literal
                    && !text.value().is_empty()
                {
                    self.0.insert(text.value());
                }
            }
        }
        Literals(&mut self.0).visit_expr(expression);
    }
}

fn test_only(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("test")
            || (attribute.path().is_ident("cfg")
                && matches!(attribute.parse_args::<syn::Meta>(),
                    Ok(syn::Meta::Path(path)) if path.is_ident("test")))
    })
}

impl<'ast> Visit<'ast> for Strings {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if let Ok(arguments) = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
            .parse2(invocation.tokens.clone())
        {
            for argument in &arguments {
                self.visit_expr(argument);
            }
        }
    }

    fn visit_item_mod(&mut self, module: &'ast ItemMod) {
        if !test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        if call.method == "text" {
            for argument in &call.args {
                self.expression(argument);
            }
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if let Expr::Path(path) = call.func.as_ref()
            && let Some(name) = path.path.segments.last()
        {
            let first = match name.ident.to_string().as_str() {
                "row_label" | "label" => Some(2),
                "section_heading" => Some(3),
                _ => None,
            };
            if let Some(first) = first {
                for argument in call.args.iter().skip(first) {
                    self.expression(argument);
                }
            }
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_match(&mut self, expression: &'ast ExprMatch) {
        // Built-in translations also cover labels assembled dynamically from stat names.
        if matches!(expression.expr.as_ref(), Expr::Path(path) if path.path.is_ident("source")) {
            for arm in &expression.arms {
                if let syn::Pat::Lit(literal) = &arm.pat {
                    self.expression(&Expr::Lit(literal.clone()));
                } else if let syn::Pat::Or(alternatives) = &arm.pat {
                    for pattern in &alternatives.cases {
                        if let syn::Pat::Lit(literal) = pattern {
                            self.expression(&Expr::Lit(literal.clone()));
                        }
                    }
                }
            }
        }
        visit::visit_expr_match(self, expression);
    }

    fn visit_expr_struct(&mut self, expression: &'ast ExprStruct) {
        for field in &expression.fields {
            if let syn::Member::Named(name) = &field.member
                && matches!(name.to_string().as_str(), "label" | "help" | "title")
            {
                self.expression(&field.expr);
            }
        }
        visit::visit_expr_struct(self, expression);
    }
}

#[cfg(not(test))]
fn read_sources(directory: &Path, strings: &mut Strings) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            read_sources(&path, strings)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path)?;
            let syntax = syn::parse_file(&source).map_err(|error| {
                format!(
                    "Cannot collect interface strings from {}: {error}",
                    path.display()
                )
            })?;
            strings.visit_file(&syntax);
        }
    }
    Ok(())
}

#[cfg(not(test))]
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src");
    let mut strings = Strings::default();
    read_sources(Path::new("src"), &mut strings)?;
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("Missing OUT_DIR")?)
        .join("interface_strings.rs");
    fs::write(
        &output,
        format!("&{:?}", strings.0.iter().collect::<Vec<_>>()),
    )?;
    println!("cargo:rustc-env=CU_INTERFACE_STRINGS={}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_includes_nested_calls_aliases_and_decoded_strings_but_not_tests() {
        let source = r##"
            fn render(names: Names) {
                format!("not a key {}", names.text(r#"A "quoted" label"#));
                names.text(if ready { "Ready" } else { "Not ready" });
                row_label(ui, names, "Parameter");
                section_heading(ui, names, "icon", "Section");
            }
            fn translate(source: &str) {
                match source { "Stat" | "Alias" => Some("not a key"), _ => None }
            }
            #[cfg(test)]
            mod tests { fn example() { names.text("Test fixture"); } }
        "##;
        let mut strings = Strings::default();
        strings.visit_file(&syn::parse_file(source).unwrap());
        assert_eq!(
            strings.0,
            [
                "A \"quoted\" label",
                "Alias",
                "Not ready",
                "Parameter",
                "Ready",
                "Section",
                "Stat"
            ]
            .map(str::to_owned)
            .into()
        );
    }
}
