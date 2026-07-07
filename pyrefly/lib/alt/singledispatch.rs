/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Solving-phase support for `functools.singledispatch`, whose typing behavior the typeshed stub
//! cannot express.

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::types::types::Type;

impl<'a, Ans: LookupAnswer> AnswersSolver<'a, Ans> {
    /// Replace a singledispatch dispatcher's `Never` element type with gradual `Any`.
    pub(crate) fn widen_singledispatch_never(&self, mut ty: Type) -> Type {
        if let Type::ClassType(ct) = &mut ty
            && (ct.has_qname("functools", "_SingleDispatchCallable")
                || ct.has_qname("singledispatch", "_SingleDispatchCallable"))
            && let [arg] = ct.targs().as_slice()
            && self.solver().force(arg.clone()).is_never()
        {
            ct.targs_mut().as_mut()[0] = Type::any_implicit();
        }
        ty
    }
}
