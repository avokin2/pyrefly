/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::sync::Arc;

use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_name::is_python_identifier;
use pyrefly_types::callable::Callable;
use pyrefly_types::callable::ParamList;
use pyrefly_types::class::Class;
use pyrefly_types::function::FuncMetadata;
use pyrefly_types::function::Function;
use pyrefly_types::function::FunctionKind;
use pyrefly_types::function::PropertyMetadata;
use pyrefly_types::function::PropertyRole;
use pyrefly_types::heap::TypeHeap;
use pyrefly_types::literal::Lit;
use pyrefly_types::tuple::Tuple;
use pyrefly_types::type_alias::TypeAlias;
use pyrefly_types::type_alias::TypeAliasStyle;
use pyrefly_types::types::CalleeKind;
use pyrefly_types::types::Type;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprCall;
use ruff_python_ast::ExprStringLiteral;
use ruff_python_ast::name::Name;
use ruff_text_size::TextRange;
use starlark_map::small_map::SmallMap;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::alt::class::class_field::ClassField;
use crate::alt::class::enums::VALUE_PROP;
use crate::alt::types::class_metadata::ClassMetadata;
use crate::alt::types::class_metadata::ClassSynthesizedField;
use crate::alt::types::class_metadata::ClassSynthesizedFields;
use crate::alt::types::class_metadata::DjangoReverseRelationIndex;
use crate::binding::binding::BindingDjangoRelations;
use crate::binding::binding::ClassFieldDefinition;
use crate::binding::binding::ExprOrBinding;
use crate::binding::binding::KeyExport;
use crate::error::collector::ErrorCollector;
use crate::types::simplify::unions;

/// Django stubs use this attribute to specify the Python type that a field should infer to
const DJANGO_PRIVATE_GET_TYPE: Name = Name::new_static("_pyi_private_get_type");

pub fn is_django_choices_subclass(bases_with_metadata: &[(Class, Arc<ClassMetadata>)]) -> bool {
    bases_with_metadata.iter().any(|(base, base_meta)| {
        base.has_toplevel_qname(ModuleName::django_models_enums().as_str(), "Choices")
            || base_meta
                .enum_metadata()
                .as_ref()
                .is_some_and(|meta| meta.is_django)
    })
}

/// Strip the label element from a Django enum tuple value.
/// Django `Choices` enums use `(value, label)` tuples; this strips the last
/// element (the label) and returns the remaining value portion.
pub fn transform_django_enum_value(ty: Type, heap: &TypeHeap) -> Type {
    match ty {
        Type::Tuple(Tuple::Concrete(elements)) if elements.len() >= 2 => {
            let value_len = elements.len() - 1;
            heap.mk_concrete_tuple(elements.into_iter().take(value_len).collect())
        }
        ty => ty,
    }
}
const CHOICES: Name = Name::new_static("choices");
const LABEL: Name = Name::new_static("label");
const LABELS: Name = Name::new_static("labels");
const VALUES: Name = Name::new_static("values");
const ID: Name = Name::new_static("id");
const PK: Name = Name::new_static("pk");
const AUTO_FIELD: Name = Name::new_static("AutoField");
const FOREIGN_KEY: Name = Name::new_static("ForeignKey");
const ONE_TO_ONE_FIELD: Name = Name::new_static("OneToOneField");
const NULL: Name = Name::new_static("null");
const BLANK: Name = Name::new_static("blank");
const THROUGH: Name = Name::new_static("through");
const CHAR_FIELD: Name = Name::new_static("CharField");
const MANY_TO_MANY_FIELD: Name = Name::new_static("ManyToManyField");
const MODEL: Name = Name::new_static("Model");
const MANYRELATEDMANAGER: Name = Name::new_static("ManyRelatedManager");
const SYMMETRICAL: Name = Name::new_static("symmetrical");
const BASEMANAGER: Name = Name::new_static("BaseManager");
const AUTH_USER_MODEL: Name = Name::new_static("AUTH_USER_MODEL");
const USER: Name = Name::new_static("User");
const MODEL_FORM: Name = Name::new_static("ModelForm");
const BASE_MODEL_FORM_SET: Name = Name::new_static("BaseModelFormSet");

/// Find a keyword argument by name and return its value expression.
fn find_keyword<'a>(call_expr: &'a ExprCall, name: &Name) -> Option<&'a Expr> {
    call_expr
        .arguments
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|n| n.as_str() == name.as_str()))
        .map(|kw| &kw.value)
}

/// Check if a keyword argument with the given name exists and has value `True`.
fn has_keyword_true(call_expr: &ExprCall, name: &Name) -> bool {
    find_keyword(call_expr, name)
        .is_some_and(|v| matches!(v, Expr::BooleanLiteral(lit) if lit.value))
}

const RELATED_NAME: Name = Name::new_static("related_name");

const RELATED_MANAGER: Name = Name::new_static("RelatedManager");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DjangoRelationKind {
    ForeignKey,
    OneToOne,
    ManyToMany,
}

impl<'a, Ans: LookupAnswer> AnswersSolver<'a, Ans> {
    pub(crate) fn apply_framework_call_specialization(
        &self,
        callee: &Type,
        call: &ExprCall,
        default: Type,
        errors: &ErrorCollector,
    ) -> Type {
        let Some(CalleeKind::Function(FunctionKind::Def(id))) = callee.callee_kind() else {
            return default;
        };
        if id.qname.module_name().as_str() == "django.forms.formsets"
            && id.qname.id().as_str() == "formset_factory"
        {
            let Some(form_expr) = call.arguments.find_argument_value("form", 0) else {
                return default;
            };
            let Some(formset_expr) = call.arguments.find_argument_value("formset", 1) else {
                return default;
            };
            let Type::ClassDef(form) = self.expr_infer(form_expr, errors) else {
                return default;
            };
            let Type::ClassDef(formset) = self.expr_infer(formset_expr, errors) else {
                return default;
            };
            let specialized =
                self.specialize(&formset, vec![self.instantiate(&form)], call.range, errors);
            return Type::Type(Box::new(specialized));
        }
        if id.qname.module_name().as_str() != "django.forms.models" {
            return default;
        }
        let Some(model_expr) = call.arguments.find_argument_value("model", 0) else {
            return default;
        };
        let Type::ClassDef(model) = self.expr_infer(model_expr, errors) else {
            return default;
        };
        match id.qname.id().as_str() {
            "modelform_factory" => {
                let Some(form_expr) = call.arguments.find_argument_value("form", 1) else {
                    return default;
                };
                let Type::ClassDef(form) = self.expr_infer(form_expr, errors) else {
                    return default;
                };
                let specialized =
                    self.specialize(&form, vec![self.instantiate(&model)], call.range, errors);
                Type::Type(Box::new(specialized))
            }
            "modelformset_factory" => {
                let form = if let Some(form_expr) = call.arguments.find_argument_value("form", 1) {
                    let Type::ClassDef(form) = self.expr_infer(form_expr, errors) else {
                        return default;
                    };
                    form
                } else {
                    let Some(form) = self.try_get_from_export(
                        ModuleName::from_str("django.forms.models"),
                        MODEL_FORM,
                    ) else {
                        return default;
                    };
                    let Type::ClassDef(form) = form.as_ref() else {
                        return default;
                    };
                    form.clone()
                };
                let specialized_form =
                    self.specialize(&form, vec![self.instantiate(&model)], call.range, errors);
                let formset =
                    if let Some(formset_expr) = call.arguments.find_argument_value("formset", 3) {
                        let Type::ClassDef(formset) = self.expr_infer(formset_expr, errors) else {
                            return default;
                        };
                        formset
                    } else {
                        let Some(formset) = self.try_get_from_export(
                            ModuleName::from_str("django.forms.models"),
                            BASE_MODEL_FORM_SET,
                        ) else {
                            return default;
                        };
                        let Type::ClassDef(formset) = formset.as_ref() else {
                            return default;
                        };
                        formset.clone()
                    };
                let specialized = self.specialize(
                    &formset,
                    vec![self.instantiate(&model), specialized_form],
                    call.range,
                    errors,
                );
                Type::Type(Box::new(specialized))
            }
            _ => default,
        }
    }

    pub(crate) fn framework_type_alias_override(
        &self,
        name: &Name,
        style: TypeAliasStyle,
    ) -> Option<Arc<TypeAlias>> {
        self.django_type_alias_override(name, style)
    }

    fn is_one_to_one_field(&self, field: &Class) -> bool {
        field.has_toplevel_qname(
            ModuleName::django_models_fields_related().as_str(),
            ONE_TO_ONE_FIELD.as_str(),
        )
    }

    pub(crate) fn django_type_alias_override(
        &self,
        name: &Name,
        style: TypeAliasStyle,
    ) -> Option<Arc<TypeAlias>> {
        if self.module().name().as_str() != "django.contrib.auth.models" || name.as_str() != "_User"
        {
            return None;
        }
        let model = self.auth_user_model()?;
        Some(Arc::new(TypeAlias::new(name.clone(), model, style)))
    }

    fn django_settings_module(&self) -> Option<ModuleName> {
        self.bindings()
            .framework()
            .option("django", "settings-module")
            .map(ModuleName::from_str)
            .or_else(|| {
                std::env::var("DJANGO_SETTINGS_MODULE")
                    .ok()
                    .map(|x| ModuleName::from_str(&x))
            })
    }

    fn auth_user_model_from_settings(&self, settings: ModuleName) -> Option<Type> {
        if !self.exports.export_exists(settings, &AUTH_USER_MODEL) {
            return self.default_auth_user_model();
        }
        let setting = self.get_from_export(settings, None, &KeyExport(AUTH_USER_MODEL));
        let Type::Literal(lit) = setting.as_ref() else {
            return None;
        };
        let Lit::Str(model_label) = &lit.value else {
            return None;
        };
        self.resolve_django_model_label(model_label, None)
    }

    fn default_auth_user_model(&self) -> Option<Type> {
        self.django_model_from_export(ModuleName::from_str("django.contrib.auth.models"), USER)
    }

    fn auth_user_model(&self) -> Option<Type> {
        if let Some(settings) = self.django_settings_module() {
            self.auth_user_model_from_settings(settings)
        } else {
            self.default_auth_user_model()
        }
    }

    fn django_model_from_export(&self, module: ModuleName, name: Name) -> Option<Type> {
        let model = self.try_get_from_export(module, name)?;
        let Type::ClassDef(_) = model.as_ref() else {
            return None;
        };
        Some(self.class_def_to_instance_type(&model))
    }

    fn resolve_django_model_label(
        &self,
        model_label: &str,
        current_class: Option<&Class>,
    ) -> Option<Type> {
        if model_label == "self" {
            return current_class.map(|class| self.instantiate(class));
        }
        let Some((app_label, model_name)) = model_label.rsplit_once('.') else {
            return self
                .django_model_from_export(current_class?.module_name(), Name::new(model_label));
        };
        // Django's built-in `auth` app label does not match its import package.
        let models_module = if app_label == "auth" {
            ModuleName::from_str("django.contrib.auth.models")
        } else {
            ModuleName::from_str(&format!("{app_label}.models"))
        };
        self.django_model_from_export(models_module, Name::new(model_name))
            .or_else(|| {
                self.django_model_from_export(current_class?.module_name(), Name::new(model_name))
            })
    }

    fn is_auth_user_model_setting(&self, expr: &Expr) -> bool {
        let Expr::Attribute(attr) = expr else {
            return false;
        };
        if attr.attr.id != AUTH_USER_MODEL {
            return false;
        }
        matches!(
            self.expr_infer(&attr.value, &self.error_swallower()),
            Type::ClassType(settings) if settings.has_qname("django.conf", "LazySettings")
        )
    }

    pub(crate) fn may_preserve_inferred_class_field_type(&self, class: &Class) -> bool {
        self.get_metadata_for_class(class).is_django_model()
    }

    /// Framework-owned class attributes may intentionally refine an annotation inherited
    /// from the framework base class. Keep the concrete manager subtype declared by a model.
    pub(crate) fn should_preserve_inferred_class_field_type(
        &self,
        class: &Class,
        ty: &Type,
    ) -> bool {
        if !self.may_preserve_inferred_class_field_type(class) {
            return false;
        }
        let Type::ClassType(manager) = ty else {
            return false;
        };
        manager.has_qname(
            ModuleName::from_str("django.db.models.manager").as_str(),
            BASEMANAGER.as_str(),
        ) || self
            .get_mro_for_class(manager.class_object())
            .ancestors(self.stdlib)
            .any(|ancestor| {
                ancestor.has_qname(
                    ModuleName::from_str("django.db.models.manager").as_str(),
                    BASEMANAGER.as_str(),
                )
            })
    }

    pub fn get_django_field_type(
        &self,
        ty: &Type,
        class: &Class,
        field_name: Option<&Name>,
        initial_value_expr: Option<&Expr>,
    ) -> Option<Type> {
        match ty {
            Type::ClassType(cls)
                if cls.has_qname(ModuleName::django_utils_functional().as_str(), "_Getter") =>
            {
                cls.targs().as_slice().first().cloned()
            }
            Type::ClassType(cls) => self.get_django_field_type_from_class(
                cls.class_object(),
                class,
                field_name,
                initial_value_expr,
            ),
            Type::ClassDef(cls) => {
                self.get_django_field_type_from_class(cls, class, field_name, initial_value_expr)
            }
            Type::Union(f) => {
                let transformed: Vec<_> = f
                    .members
                    .iter()
                    .map(|variant| {
                        self.get_django_field_type(variant, class, field_name, initial_value_expr)
                            .unwrap_or_else(|| variant.clone())
                    })
                    .collect();

                if transformed != f.members.to_vec() {
                    Some(unions(transformed, self.heap))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn get_django_field_type_from_class(
        &self,
        field: &Class,
        class: &Class,
        field_name: Option<&Name>,
        initial_value_expr: Option<&Expr>,
    ) -> Option<Type> {
        if !(self.get_metadata_for_class(class).is_django_model()
            && self.inherits_from_django_field(field))
        {
            return None;
        }

        let base_type = if field_name.is_some()
            && let Some(e) = initial_value_expr
            && let Some(call_expr) = e.as_call_expr()
            && let Some(to_expr) = call_expr.arguments.args.first()
            && let Some(model_type) = self.resolve_target(to_expr, class)
        {
            if self.is_foreign_key_like_field(field) {
                Some(model_type)
            } else if self.is_many_to_many_field(field) {
                let through_type = find_keyword(call_expr, &THROUGH)
                    .and_then(|expr| self.resolve_target(expr, class));
                return self.get_manager_type(model_type, through_type);
            } else {
                None
            }
        } else {
            None
        };

        let base_type = base_type.or_else(|| {
            self.get_class_member(field, &DJANGO_PRIVATE_GET_TYPE)
                .map(|field| field.ty())
        })?;

        let maybe_narrowed_type =
            self.narrow_charfield_choices(field, initial_value_expr, base_type);

        if let Some(e) = initial_value_expr
            && let Some(call_expr) = e.as_call_expr()
            && self.is_django_field_nullable(call_expr)
        {
            Some(self.union(maybe_narrowed_type, self.heap.mk_none()))
        } else {
            Some(maybe_narrowed_type)
        }
    }

    /// Narrow CharField with inline choices to a Literal type.
    /// Only blank=False is supported for now.
    fn narrow_charfield_choices(
        &self,
        field: &Class,
        initial_value_expr: Option<&Expr>,
        base_type: Type,
    ) -> Type {
        if let Some(e) = initial_value_expr
            && let Some(call_expr) = e.as_call_expr()
            && self.is_char_field(field)
            && !self.is_django_field_blank(call_expr)
            && let Some(literal_type) = self.extract_charfield_choices_literal_type(call_expr)
        {
            literal_type
        } else {
            base_type
        }
    }

    /// Check if a class inherits from Django's Field class
    fn inherits_from_django_field(&self, cls: &Class) -> bool {
        self.get_mro_for_class(cls)
            .ancestors(self.stdlib)
            .any(|ancestor| {
                ancestor.has_qname(ModuleName::django_models_fields().as_str(), "Field")
            })
    }

    fn get_manager_type(
        &self,
        target_model_type: Type,
        through_model_type: Option<Type>,
    ) -> Option<Type> {
        let through_model_type = if let Some(through_model_type) = through_model_type {
            through_model_type
        } else {
            let model_class = self.try_get_from_export(ModuleName::django_models(), MODEL)?;
            self.class_def_to_instance_type(&model_class)
        };
        self.specialize_manager_type(
            MANYRELATEDMANAGER,
            vec![target_model_type, through_model_type],
        )
    }

    fn get_related_manager_type(&self, target_model_type: Type) -> Option<Type> {
        self.specialize_manager_type(RELATED_MANAGER, vec![target_model_type])
    }

    fn specialize_manager_type(&self, name: Name, type_args: Vec<Type>) -> Option<Type> {
        let manager_class_type =
            self.try_get_from_export(ModuleName::django_models_fields_related_descriptors(), name)?;
        let Type::ClassDef(manager_class) = manager_class_type.as_ref() else {
            return None;
        };
        Some(self.specialize(
            manager_class,
            type_args,
            TextRange::default(),
            &self.error_swallower(),
        ))
    }

    fn resolve_target(&self, to_expr: &Expr, class: &Class) -> Option<Type> {
        if self.is_auth_user_model_setting(to_expr) {
            return self.auth_user_model();
        }
        match to_expr {
            // Use expr_infer to resolve the model in the current scope.
            Expr::Name(_) | Expr::Attribute(_) => {
                let model_type = self.expr_infer(to_expr, &self.error_swallower());
                if let Type::Literal(lit) = &model_type
                    && let Lit::Str(model_label) = &lit.value
                {
                    self.resolve_django_model_label(model_label, Some(class))
                } else {
                    Some(self.class_def_to_instance_type(&model_type))
                }
            }
            Expr::StringLiteral(ExprStringLiteral { value, .. }) => {
                self.resolve_django_model_label(value.to_str(), Some(class))
            }
            // we may have to extend this function to handle different kinds of fields in the future
            _ => None,
        }
    }

    fn class_def_to_instance_type(&self, ty: &Type) -> Type {
        if let Type::ClassDef(class) = ty {
            self.instantiate(class)
        } else {
            ty.clone()
        }
    }

    pub fn is_foreign_key_like_field(&self, field: &Class) -> bool {
        let module = ModuleName::django_models_fields_related();
        field.has_toplevel_qname(module.as_str(), FOREIGN_KEY.as_str())
            || field.has_toplevel_qname(module.as_str(), ONE_TO_ONE_FIELD.as_str())
    }

    pub fn is_many_to_many_field(&self, field: &Class) -> bool {
        field.has_toplevel_qname(
            ModuleName::django_models_fields_related().as_str(),
            MANY_TO_MANY_FIELD.as_str(),
        )
    }

    pub fn get_django_enum_synthesized_fields(
        &self,
        cls: &Class,
    ) -> Option<ClassSynthesizedFields> {
        let metadata = self.get_metadata_for_class(cls);
        let enum_metadata = metadata.enum_metadata()?;
        if !enum_metadata.is_django {
            return None;
        }

        let enum_members = self.get_enum_members(cls);

        let mut label_types: Vec<Type> = enum_members
            .iter()
            .filter_map(|lit| {
                if let Lit::Enum(lit_enum) = lit
                    && let Type::Tuple(Tuple::Concrete(elements)) = &lit_enum.ty
                    && elements.len() >= 2
                {
                    Some(
                        elements[elements.len() - 1]
                            .clone()
                            .promote_implicit_literals(self.stdlib),
                    )
                } else {
                    None
                }
            })
            .collect();

        if label_types.is_empty() || label_types.len() < enum_members.len() {
            // Members without a custom label type have default label type str.
            label_types.push(self.heap.mk_class_type(self.stdlib.str().clone()));
        }

        // Also include the type of __empty__ field if it exists, since it contributes to label types
        let empty_name = Name::new_static("__empty__");
        let has_empty = if let Some(field) = self.get_class_member(cls, &empty_name) {
            label_types.push(field.ty());
            true
        } else {
            false
        };

        let label_type = self.unions(label_types);

        let base_value_attr = self.get_enum_or_instance_attribute(
            &self.as_class_type_unchecked(cls),
            &metadata,
            &VALUE_PROP,
        );
        let base_value_type = base_value_attr
            .and_then(|attr| {
                self.resolve_get_class_attr(
                    &VALUE_PROP,
                    attr,
                    TextRange::default(),
                    &self.error_swallower(),
                    None,
                )
                .ok()
            })
            .unwrap_or_else(|| self.heap.mk_any_implicit());

        // if value is optional, make the type optional
        let values_type = if has_empty {
            self.union(base_value_type.clone(), self.heap.mk_none())
        } else {
            base_value_type
        };

        let mut fields = SmallMap::new();

        let field_specs = [
            (
                LABELS,
                self.heap
                    .mk_class_type(self.stdlib.list(label_type.clone())),
            ),
            (LABEL, self.property(cls, LABEL, label_type.clone())),
            (
                VALUES,
                self.heap
                    .mk_class_type(self.stdlib.list(values_type.clone())),
            ),
            (
                CHOICES,
                self.heap.mk_class_type(
                    self.stdlib
                        .list(self.heap.mk_concrete_tuple(vec![values_type, label_type])),
                ),
            ),
        ];

        for (name, ty) in field_specs {
            fields.insert(name, ClassSynthesizedField::new(ty));
        }

        Some(ClassSynthesizedFields::new(fields))
    }

    fn property(&self, cls: &Class, name: Name, ty: Type) -> Type {
        let signature = Callable::list(ParamList::new(vec![self.class_self_param(cls, false)]), ty);
        let mut metadata = FuncMetadata::method(cls, name);
        metadata.flags.property_metadata = Some(PropertyMetadata {
            role: PropertyRole::Getter,
            getter: self.heap.mk_any_error(),
            setter: None,
            has_deleter: false,
        });
        self.heap.mk_function(Function {
            signature,
            metadata,
        })
    }

    /// Get the primary key field type for a Django model.
    /// Returns a tuple of (pk_type, has_custom_pk) where has_custom_pk indicates
    /// whether the model has a custom primary key field defined.
    fn get_pk_field_type(&self, model: &Class) -> Option<(Type, bool)> {
        let metadata = self.get_metadata_for_class(model);

        if let Some(pk_field_name) = metadata
            .django_model_metadata()
            .and_then(|dm| dm.custom_primary_key_field.as_ref())
        {
            let instance_type = self.heap.mk_class_type(self.as_class_type_unchecked(model));
            let pk_type = self.attr_infer_for_type(
                &instance_type,
                pk_field_name,
                TextRange::default(),
                &self.error_swallower(),
                None,
            );
            Some((pk_type, true))
        } else {
            // No custom pk, use default AutoField type
            let auto_field_type =
                self.try_get_from_export(ModuleName::django_models_fields(), AUTO_FIELD)?;
            self.get_django_field_type(&auto_field_type, model, None, None)
                .map(|ty| (ty, false))
        }
    }

    fn is_django_field_nullable(&self, call_expr: &ExprCall) -> bool {
        has_keyword_true(call_expr, &NULL)
    }

    /// Check if a Django field has a `choices` argument.
    pub fn has_django_field_choices(&self, call_expr: &ExprCall) -> bool {
        find_keyword(call_expr, &CHOICES).is_some()
    }

    /// Check if a Django field has `blank=True`.
    fn is_django_field_blank(&self, call_expr: &ExprCall) -> bool {
        has_keyword_true(call_expr, &BLANK)
    }

    /// Check if a Django field is a CharField.
    fn is_char_field(&self, field: &Class) -> bool {
        field.has_toplevel_qname(
            ModuleName::django_models_fields().as_str(),
            CHAR_FIELD.as_str(),
        )
    }

    /// Extract a Literal type from CharField choices.
    ///
    /// Only supports inline tuple-of-tuples: choices=(("A", "Label A"), ("B", "Label B"), ...)
    /// Returns None if:
    /// - choices is not found
    /// - format is not the simple inline tuple-of-tuples
    /// - any value is not a string literal
    fn extract_charfield_choices_literal_type(&self, call_expr: &ExprCall) -> Option<Type> {
        let choices_value = find_keyword(call_expr, &CHOICES)?;

        let elements = &choices_value.as_tuple_expr()?.elts;

        let mut choice_literals = Vec::new();
        for element in elements {
            let inner_tuple = element.as_tuple_expr()?;
            let string_lit = inner_tuple.elts.first()?.as_string_literal_expr()?;
            choice_literals.push(Lit::from_string_literal(string_lit)?.to_implicit_type());
        }

        if choice_literals.is_empty() {
            None
        } else {
            Some(self.unions(choice_literals))
        }
    }

    /// Create a get_FOO_display method signature for a field with choices.
    /// The method takes self and returns str.
    fn get_display_method(&self, cls: &Class, method_name: &Name) -> ClassSynthesizedField {
        let params = vec![self.class_self_param(cls, false)];
        let ret = self.heap.mk_class_type(self.stdlib.str().clone());
        ClassSynthesizedField::new(self.heap.mk_function(Function {
            signature: Callable::list(ParamList::new(params), ret),
            metadata: FuncMetadata::method(cls, method_name.clone()),
        }))
    }

    /// Returns the primary key type of the related model.
    fn get_foreign_key_id_type(&self, class_field: &ClassField) -> Option<Type> {
        // Check if this is a ForeignKey field using the cached metadata
        if !class_field.is_foreign_key() {
            return None;
        }

        // Get the related model type from the field
        let ty = class_field.ty();
        let (related_cls, is_foreign_key_nullable) = match ty {
            Type::Union(f) => {
                // Nullable foreign key: extract the class type from the union
                let cls = f.members.iter().find_map(|variant| match variant {
                    Type::ClassType(cls) => Some(cls.clone()),
                    _ => None,
                })?;
                (cls, true)
            }
            Type::ClassType(cls) => (cls, false),
            _ => return None,
        };

        // Get the pk type from the related model and make it nullable if needed
        let (pk_type, _) = self.get_pk_field_type(related_cls.class_object())?;
        if is_foreign_key_nullable {
            Some(self.union(pk_type, self.heap.mk_none()))
        } else {
            Some(pk_type)
        }
    }

    pub fn get_django_model_synthesized_fields(
        &self,
        cls: &Class,
    ) -> Option<ClassSynthesizedFields> {
        let metadata = self.get_metadata_for_class(cls);
        let django_metadata = metadata.django_model_metadata()?;

        let mut fields = SmallMap::new();

        if let Some((pk_type, has_custom_pk)) = self.get_pk_field_type(cls) {
            if !has_custom_pk {
                // No custom pk, so synthesize an id field
                fields.insert(ID, ClassSynthesizedField::new(pk_type.clone()));
            }
            fields.insert(PK, ClassSynthesizedField::new(pk_type));
        }

        // Synthesize `<field_name>_id` fields for ForeignKey and OneToOneField fields.
        // We use field names cached in metadata (detected during binding phase)
        // to avoid triggering type resolution during synthesis, which can cause cycles.
        for field_name in &django_metadata.foreign_key_like_fields {
            if let Some(class_field) = self.get_field_from_current_class_only(cls, field_name)
                && let Some(fk_id_type) = self.get_foreign_key_id_type(&class_field)
            {
                let id_field_name = Name::new(format!("{}_id", field_name));
                fields.insert(id_field_name, ClassSynthesizedField::new(fk_id_type));
            }
        }

        // Synthesize `get_<field_name>_display()` methods for fields with choices.
        // Same caching strategy as FK fields above.
        for field_name in &django_metadata.fields_with_choices {
            let method_name = Name::new(format!("get_{}_display", field_name));
            fields.insert(
                method_name.clone(),
                self.get_display_method(cls, &method_name),
            );
        }

        let reverse_relations = self.django_reverse_relations_index();
        if let Some(reverse_fields) = reverse_relations.get(cls) {
            for (name, field) in reverse_fields.fields() {
                fields.insert(name.clone(), field.clone());
            }
        }

        Some(ClassSynthesizedFields::new(fields))
    }

    pub fn solve_django_reverse_relations(
        &self,
        binding: &BindingDjangoRelations,
        _range: TextRange,
        _errors: &ErrorCollector,
    ) -> Arc<DjangoReverseRelationIndex> {
        let mut per_class = SmallMap::new();

        for candidate_class in &binding.classes {
            let Some(source_class) = &self.get_idx(candidate_class.class_idx).0 else {
                continue;
            };
            if !self.get_metadata_for_class(source_class).is_django_model() {
                continue;
            }

            for field_idx in &candidate_class.fields {
                let binding = self.bindings().get(*field_idx);
                let ClassFieldDefinition::AssignedInBody { value, .. } = &binding.definition else {
                    continue;
                };
                let ExprOrBinding::Expr(expr) = value.as_ref() else {
                    continue;
                };
                let Some(call_expr) = expr.as_call_expr() else {
                    continue;
                };

                let Some(relation_kind) = self.django_relation_kind(expr) else {
                    continue;
                };

                let Some(to_expr) = call_expr.arguments.args.first() else {
                    continue;
                };
                let Some(target_type) = self.resolve_target(to_expr, source_class) else {
                    continue;
                };
                let target_class = match &target_type {
                    Type::ClassType(cls_type) => cls_type.class_object(),
                    Type::ClassDef(class_def) => class_def,
                    _ => continue,
                };
                if relation_kind == DjangoRelationKind::ManyToMany
                    && self.is_symmetrical_self_m2m(call_expr, source_class, target_class)
                {
                    continue;
                }
                let Some(related_name) =
                    self.django_related_name(call_expr, source_class, relation_kind)
                else {
                    continue;
                };
                let Some(related_type) =
                    self.django_reverse_field_type(relation_kind, source_class, call_expr)
                else {
                    continue;
                };

                per_class
                    .entry(target_class.clone())
                    .or_insert_with(SmallMap::new)
                    .insert(related_name, ClassSynthesizedField::new(related_type));
            }
        }

        let mut reverse_relations = SmallMap::new();
        for (class, fields) in per_class.into_iter_hashed() {
            reverse_relations.insert_hashed(class, ClassSynthesizedFields::new(fields));
        }

        Arc::new(DjangoReverseRelationIndex::new(reverse_relations))
    }

    fn django_relation_kind(&self, expr: &Expr) -> Option<DjangoRelationKind> {
        let ty = self.expr_infer(expr, &self.error_swallower());
        let field_class = match &ty {
            Type::ClassType(cls) => cls.class_object(),
            Type::ClassDef(cls) => cls,
            _ => return None,
        };

        if self.is_one_to_one_field(field_class) {
            Some(DjangoRelationKind::OneToOne)
        } else if self.is_many_to_many_field(field_class) {
            Some(DjangoRelationKind::ManyToMany)
        } else if self.is_foreign_key_like_field(field_class) {
            Some(DjangoRelationKind::ForeignKey)
        } else {
            None
        }
    }

    fn django_reverse_field_type(
        &self,
        relation_kind: DjangoRelationKind,
        source_class: &Class,
        call_expr: &ExprCall,
    ) -> Option<Type> {
        let source_type = self.instantiate(source_class);
        match relation_kind {
            DjangoRelationKind::ForeignKey => self.get_related_manager_type(source_type),
            DjangoRelationKind::OneToOne => Some(source_type),
            DjangoRelationKind::ManyToMany => {
                let through_type = find_keyword(call_expr, &THROUGH)
                    .and_then(|expr| self.resolve_target(expr, source_class));
                self.get_manager_type(source_type, through_type)
            }
        }
    }

    fn django_related_name(
        &self,
        call_expr: &ExprCall,
        source_class: &Class,
        relation_kind: DjangoRelationKind,
    ) -> Option<Name> {
        match find_keyword(call_expr, &RELATED_NAME) {
            None | Some(Expr::NoneLiteral(_)) => {
                Some(self.django_default_related_name(source_class, relation_kind))
            }
            Some(Expr::StringLiteral(lit)) => {
                self.format_related_name(lit.value.to_str(), source_class)
            }
            _ => None,
        }
    }

    fn django_default_related_name(
        &self,
        source_class: &Class,
        relation_kind: DjangoRelationKind,
    ) -> Name {
        let mut name = source_class.name().as_str().to_lowercase();
        if matches!(
            relation_kind,
            DjangoRelationKind::ForeignKey | DjangoRelationKind::ManyToMany
        ) {
            name.push_str("_set");
        }
        Name::new(name)
    }

    fn format_related_name(&self, raw: &str, source_class: &Class) -> Option<Name> {
        if raw.ends_with('+') {
            return None;
        }

        let class_name = source_class.name().as_str().to_lowercase();
        let mut substituted = raw.replace("%(class)s", &class_name);
        if substituted.contains("%(app_label)s") {
            let module_name = source_class.module_name();
            let mut module_parts = module_name.as_str().rsplit('.');
            let mut child = module_parts
                .next()
                .expect("rsplit always yields at least one element");
            let mut app_label = None;
            for parent in module_parts {
                if child == "models" {
                    app_label = Some(parent);
                    break;
                }
                child = parent;
            }
            substituted = substituted.replace("%(app_label)s", &app_label?.to_lowercase());
        }

        // Django rejects a related name that is not a valid identifier. This also discards
        // names with unsubstituted placeholders, which are never valid identifiers.
        if !is_python_identifier(&substituted) {
            return None;
        }

        Some(Name::new(substituted))
    }

    fn is_symmetrical_self_m2m(
        &self,
        call_expr: &ExprCall,
        source_class: &Class,
        target_class: &Class,
    ) -> bool {
        if source_class != target_class {
            return false;
        }
        // Suppress the reverse accessor only when symmetry is statically known.
        match find_keyword(call_expr, &SYMMETRICAL) {
            None => true,
            Some(Expr::NoneLiteral(_)) => true,
            Some(Expr::BooleanLiteral(lit)) => lit.value,
            Some(_) => false,
        }
    }
}
