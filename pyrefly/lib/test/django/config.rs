/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;
use crate::test::django::util::django_env;
use crate::test::util::testcase_for_macro;

django_testcase!(
    test_model,
    r#"
from django.apps.config import AppConfig
from typing_extensions import assert_type

class FooConfig(AppConfig):
    name = "foo"
    default_auto_field = "django.db.models.BigAutoField"

assert_type( 
    FooConfig.default_auto_field, str 
) 
"#,
);

#[test]
fn test_auth_user_model_configures_http_request_user() -> anyhow::Result<()> {
    let mut env = django_env().with_framework_option(
        "django",
        "settings-module",
        "project.settings",
    );
    env.add("project.settings", "AUTH_USER_MODEL = 'accounts.User'");
    env.add(
        "accounts.models",
        r#"
from django.db.models import Model

class User(Model):
    username: str
"#,
    );
    testcase_for_macro(
        env,
        r#"
from django.contrib.auth.models import AnonymousUser
from django.http import HttpRequest
from accounts.models import User
from typing_extensions import assert_type

request = HttpRequest()
assert_type(request.user, User | AnonymousUser)
"#,
        file!(),
        line!(),
    )
}
