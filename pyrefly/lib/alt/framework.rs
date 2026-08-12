/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::sync::Arc;

use pyrefly_types::type_alias::TypeAlias;
use pyrefly_types::type_alias::TypeAliasStyle;
use ruff_python_ast::name::Name;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;

impl<'a, Ans: LookupAnswer> AnswersSolver<'a, Ans> {
    pub(crate) fn framework_type_alias_override(
        &self,
        name: &Name,
        style: TypeAliasStyle,
    ) -> Option<Arc<TypeAlias>> {
        self.django_type_alias_override(name, style)
    }
}
