use super::util::Scope;
use crate::pass::Pass;
use ast::*;
use swc_common::{Fold, Visit, VisitWith};

pub fn import_analyzer() -> impl Pass {
    ImportAnalyzer {
        scope: Default::default(),
    }
}

/// Inject required helpers methods **for** module transform passes.
struct ImportAnalyzer {
    scope: Scope,
}

impl Fold<Module> for ImportAnalyzer {
    fn fold(&mut self, module: Module) -> Module {
        module.visit_with(self);

        for (_, ty) in self.scope.import_types.drain() {
            match ty {
                true => {
                    enable_helper!(interop_require_wildcard);
                }
                false => {
                    enable_helper!(interop_require_default);
                }
            }
        }

        module
    }
}

impl Visit<ExportAll> for ImportAnalyzer {
    fn visit(&mut self, export: &ExportAll) {
        *self
            .scope
            .import_types
            .entry(export.src.value.clone())
            .or_default() = true
    }
}

impl Visit<NamedExport> for ImportAnalyzer {
    fn visit(&mut self, export: &NamedExport) {
        for &NamedExportSpecifier { ref orig, .. } in export.specifiers.iter().map(|e| match *e {
            ExportSpecifier::Named(ref e) => e,
            _ => unreachable!("export default from 'foo'; should be removed by previous pass"),
        }) {
            let is_import_default = orig.sym == js_word!("default");

            if let Some(ref src) = export.src {
                if is_import_default {
                    self.scope
                        .import_types
                        .entry(src.value.clone())
                        .or_insert(false);
                } else {
                    self.scope
                        .import_types
                        .entry(src.value.clone())
                        .and_modify(|v| *v = true);
                }
            }
        }
    }
}

impl Visit<ImportDecl> for ImportAnalyzer {
    fn visit(&mut self, import: &ImportDecl) {
        if import.specifiers.is_empty() {
            // import 'foo';
            //   -> require('foo');
        } else if import.specifiers.len() == 1
            && match import.specifiers[0] {
                ImportSpecifier::Namespace(..) => true,
                _ => false,
            }
        {
            if &*import.src.value != "@swc/helpers" {
                self.scope
                    .import_types
                    .insert(import.src.value.clone(), true);
            }
        } else {
            for s in &import.specifiers {
                match *s {
                    ImportSpecifier::Namespace(..) => unreachable!(
                        "import * as foo cannot be used with other type of import specifiers"
                    ),
                    ImportSpecifier::Default(_) => {
                        self.scope
                            .import_types
                            .entry(import.src.value.clone())
                            .or_insert(false);
                    }
                    ImportSpecifier::Specific(ref i) => {
                        let ImportSpecific {
                            ref local,
                            ref imported,
                            ..
                        } = *i;
                        let name = imported
                            .as_ref()
                            .map(|i| i.sym.clone())
                            .unwrap_or_else(|| local.sym.clone());
                        let is_default = name == js_word!("default");

                        if is_default {
                            self.scope
                                .import_types
                                .entry(import.src.value.clone())
                                .or_insert(false);
                        } else {
                            self.scope
                                .import_types
                                .entry(import.src.value.clone())
                                .and_modify(|v| *v = true);
                        }
                    }
                }
            }
        }
    }
}
