/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Solving-phase support for `functools.singledispatch`, whose typing behavior the typeshed stub
//! cannot express.

use pyrefly_types::class::Class;
use ruff_text_size::TextRange;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::config::error_kind::ErrorKind;
use crate::error::collector::ErrorCollector;
use crate::types::callable::Param;
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
    /// Whether `ty` is the dispatcher produced by `@singledispatch` (a `_SingleDispatchCallable`),
    /// from either the stdlib or the `singledispatch` backport.
    fn is_singledispatch_dispatcher(ty: &Type) -> bool {
        matches!(ty, Type::ClassType(ct)
        if ct.has_qname("functools", "_SingleDispatchCallable")
            || ct.has_qname("singledispatch", "_SingleDispatchCallable"))
    }

    /// Replace a singledispatch dispatcher's `Never` element type with gradual `Any`.
    pub(crate) fn widen_singledispatch_never(&self, mut ty: Type) -> Type {
        if Self::is_singledispatch_dispatcher(&ty)
            && let Type::ClassType(ct) = &mut ty
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
}
