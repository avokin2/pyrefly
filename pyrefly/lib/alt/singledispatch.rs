/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Solving-phase support for `functools.singledispatch`, whose typing behavior the typeshed stub
//! cannot express.

use pyrefly_types::class::Class;
use pyrefly_types::class::ClassType;
use pyrefly_types::types::BoundMethodType;
use ruff_python_ast::name::Name;
use ruff_text_size::TextRange;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::config::error_kind::ErrorKind;
use crate::error::collector::ErrorCollector;
use crate::types::callable::Callable;
use crate::types::callable::FuncFlags;
use crate::types::callable::FuncMetadata;
use crate::types::callable::Function;
use crate::types::callable::FunctionKind;
use crate::types::callable::Param;
use crate::types::callable::Params;
use crate::types::types::Type;

/// Definition-site facts about a decorated function, used to validate it when it is a
/// `@singledispatch` dispatcher.
pub(crate) struct DispatcherDef<'a> {
    pub params: &'a [Param],
    pub id_range: TextRange,
    pub defining_cls: Option<&'a Class>,
    pub is_staticmethod: bool,
}

impl<'a, Ans: LookupAnswer> AnswersSolver<'a, Ans> {
    /// Whether `ct` is a `@singledispatch` dispatcher class (`_SingleDispatchCallable`)
    fn is_singledispatch_class(ct: &ClassType) -> bool {
        ct.has_qname("functools", "_SingleDispatchCallable")
            || ct.has_qname("singledispatch", "_SingleDispatchCallable")
    }

    /// Whether `ty` is a `@singledispatch` dispatcher, as either the raw `_SingleDispatchCallable`
    /// instance or the callback-protocol function it is rewritten into.
    pub(crate) fn is_singledispatch_dispatcher(ty: &Type) -> bool {
        match ty {
            Type::ClassType(ct) => Self::is_singledispatch_class(ct),
            Type::Function(f) => matches!(
                &f.metadata.kind,
                FunctionKind::CallbackProtocol(cls) if Self::is_singledispatch_class(cls)
            ),
            _ => false,
        }
    }

    /// Model the dispatcher as a callback protocol over the fallback signature, so calls are checked
    /// against the fallback while `.register`/`.dispatch` and the revealed type stay `_SingleDispatchCallable`.
    pub(crate) fn singledispatch_dispatcher_as_callback(
        &self,
        ty: Type,
        original_decoratee: &Type,
    ) -> Type {
        if let Type::ClassType(ct) = &ty
            && Self::is_singledispatch_class(ct)
            && let Some(mut signature) = original_decoratee.clone().to_callable()
        {
            // Use the element type `_T` as the return (normally identical to the fallback return),
            // since `_T` reflects later normalization of the element such as an inferred `Never` -> `Any`.
            if let [ret] = ct.targs().as_slice() {
                signature.ret = ret.clone();
            }
            return self.heap.mk_function(Function {
                signature,
                metadata: FuncMetadata {
                    kind: FunctionKind::CallbackProtocol(Box::new(ct.clone())),
                    flags: FuncFlags::default(),
                },
            });
        }
        ty
    }

    /// For checking a dispatcher call only, widen the dispatch (first) parameter to `Any` so any
    /// dispatched argument is accepted; a parameter mentioning a type variable is left intact.
    pub(crate) fn widen_singledispatch_dispatch_param(&self, ty: Type) -> Type {
        if let Type::Function(f) = &ty
            && let FunctionKind::CallbackProtocol(cls) = &f.metadata.kind
            && Self::is_singledispatch_class(cls)
        {
            let mut function = (**f).clone();
            if let Params::List(params) = &mut function.signature.params
                && let Some(dispatch_ty) = params.items_mut().iter_mut().find_map(|p| match p {
                    Param::PosOnly(_, t, _) | Param::Pos(_, t, _) | Param::Varargs(_, t) => Some(t),
                    _ => None,
                })
            {
                let mut mentions_tvar = false;
                dispatch_ty.for_each_quantified(&mut |_| mentions_tvar = true);
                if !mentions_tvar {
                    *dispatch_ty = Type::any_implicit();
                    return self.heap.mk_function(function);
                }
            }
        }
        ty
    }

    /// Replace a singledispatch dispatcher's `Never` element type with gradual `Any`.
    pub(crate) fn widen_singledispatch_never(&self, mut ty: Type) -> Type {
        if let Type::ClassType(ct) = &mut ty
            && Self::is_singledispatch_class(ct)
            && let [arg] = ct.targs().as_slice()
            && self.solver().force(arg.clone()).is_never()
        {
            ct.targs_mut().as_mut()[0] = Type::any_implicit();
        }
        ty
    }

    /// Dispatch happens on the first positional parameter, so a `singledispatch` fallback must
    /// have one.
    pub(crate) fn validate_singledispatch_dispatcher_signature(
        &self,
        ty: &Type,
        def: DispatcherDef,
        errors: &ErrorCollector,
    ) {
        if !Self::is_singledispatch_dispatcher(ty) {
            return;
        }
        let skip_self = def.defining_cls.is_some()
            && !def.is_staticmethod
            && matches!(
                def.params.first(),
                Some(Param::Pos(..) | Param::PosOnly(..))
            );
        let message = match def.params.get(skip_self as usize) {
            None => "Singledispatch function requires at least one parameter",
            Some(Param::KwOnly(..) | Param::Kwargs(..)) => {
                "First parameter of a singledispatch function must be positional"
            }
            Some(_) => return,
        };
        self.error(
            errors,
            def.id_range,
            ErrorKind::BadFunctionDefinition,
            message.to_owned(),
        );
    }

    /// The fallback first-parameter type carried by a tagged `singledispatch` `register` method,
    /// regardless of any bound-method wrapping.
    pub(crate) fn singledispatch_register_first(ty: &Type) -> Option<Type> {
        let kind = match ty {
            Type::Function(f) => &f.metadata.kind,
            Type::Overload(o) => &o.metadata.kind,
            Type::BoundMethod(bm) => match &bm.func {
                BoundMethodType::Function(f) => &f.metadata.kind,
                BoundMethodType::Forall(fa) => &fa.body.metadata.kind,
                BoundMethodType::Overload(o) => &o.metadata.kind,
            },
            _ => return None,
        };
        match kind {
            FunctionKind::SingleDispatchRegister(first) => Some((**first).clone()),
            _ => None,
        }
    }

    /// The dispatch type of a singledispatch signature: its first positional parameter, or the
    /// element type of a leading `*args` (dispatch happens on the first runtime argument either way).
    pub(crate) fn first_positional_param_type(sig: &Callable) -> Option<Type> {
        let Params::List(params) = &sig.params else {
            return None;
        };
        params.items().iter().find_map(|p| match p {
            Param::PosOnly(_, t, _) | Param::Pos(_, t, _) | Param::Varargs(_, t) => Some(t.clone()),
            _ => None,
        })
    }

    /// A registered impl can only be dispatched to if its dispatch type is a subtype of the fallback's
    /// first parameter.
    pub(crate) fn check_singledispatch_register(
        &self,
        dispatch_ty: &Type,
        fallback_first: &Type,
        range: TextRange,
        errors: &ErrorCollector,
    ) {
        if !self.is_subset_eq(dispatch_ty, fallback_first) {
            self.error(
                errors,
                range,
                ErrorKind::BadSingledispatchRegister,
                format!(
                    "Dispatch type `{}` is not a subtype of fallback first argument type `{}`",
                    self.for_display(dispatch_ty.clone()),
                    self.for_display(fallback_first.clone()),
                ),
            );
        }
    }

    /// Accessing `.register` collapses the base to `_SingleDispatchCallable`, losing the fallback's
    /// first parameter; tag the returned method with that type so the dispatch type can be validated.
    pub(crate) fn tag_singledispatch_register(
        &self,
        base: &Type,
        attr_name: &Name,
        mut ty: Type,
    ) -> Type {
        if attr_name.as_str() == "register"
            && let Type::Function(f) = base
            && let FunctionKind::CallbackProtocol(cls) = &f.metadata.kind
            && matches!(
                cls.qname().module_name().as_str(),
                "functools" | "singledispatch"
            )
            && cls.name().as_str() == "_SingleDispatchCallable"
            && let Some(first) = Self::first_positional_param_type(&f.signature)
        {
            ty.transform_toplevel_func_metadata(|m| {
                m.kind = FunctionKind::SingleDispatchRegister(Box::new(first.clone()));
            });
        }
        ty
    }
}
