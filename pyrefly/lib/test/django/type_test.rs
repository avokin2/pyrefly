/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;
use crate::test::django::util::django_env;
use crate::test::util::TestEnv;
use crate::testcase;

django_testcase!(
    test_modelformset_factory,
    r#"
from typing import assert_type
from django.db import models
from django.forms import BaseModelFormSet, ModelForm, modelformset_factory

class Eggs(models.Model):
    age_1 = models.IntegerField()

expr = modelformset_factory(Eggs, fields="__all__")
assert_type(expr, type[BaseModelFormSet[Eggs, ModelForm[Eggs]]])
"#,
);

django_testcase!(
    test_modelformset_factory_with_form,
    r#"
from typing import assert_type
from django.db import models
from django.forms import BaseModelFormSet, ModelForm, modelformset_factory

class Eggs(models.Model):
    pass

class Spam[M: models.Model](ModelForm[M]):
    pass

expr = modelformset_factory(Eggs, Spam, fields="__all__")
assert_type(expr, type[BaseModelFormSet[Eggs, Spam[Eggs]]])
"#,
);

django_testcase!(
    test_modelformset_factory_with_formset,
    r#"
from typing import assert_type
from django.db import models
from django.forms import ModelForm, BaseModelFormSet, modelformset_factory

class Eggs(models.Model):
    pass

class Spam[M: models.Model](ModelForm[M]):
    pass

class Ham[M: models.Model, ModelFormT: ModelForm](BaseModelFormSet[M, ModelFormT]):
    pass

expr = modelformset_factory(Eggs, Spam, formset=Ham, fields="__all__")
assert_type(expr, type[Ham[Eggs, Spam[Eggs]]])
"#,
);

django_testcase!(
    test_modelform_factory,
    r#"
from typing import assert_type
from django.db import models
from django.forms import ModelForm, modelform_factory

class Eggs(models.Model): ...

expr = modelform_factory(Eggs)
assert_type(expr, type[ModelForm[Eggs]])
"#,
);

django_testcase!(
    test_modelform_factory_with_form,
    r#"
from typing import assert_type
from django.db import models
from django.forms import ModelForm, modelform_factory

class Eggs(models.Model): ...

class Spam[M: models.Model](ModelForm[M]): ...

expr = modelform_factory(Eggs, Spam)
assert_type(expr, type[Spam[Eggs]])
"#,
);

django_testcase!(
    test_formset_factory,
    r#"
from typing import assert_type
from django.forms import BaseForm, BaseFormSet, formset_factory

class Eggs(BaseForm): ...

expr = formset_factory(Eggs)
assert_type(expr, type[BaseFormSet[Eggs]])
"#,
);

django_testcase!(
    test_formset_factory_with_formset,
    r#"
from typing import assert_type
from django.forms import BaseForm, BaseFormSet, formset_factory

class Eggs(BaseForm): ...

class Spam[F: BaseForm](BaseFormSet[F]): ...

expr = formset_factory(Eggs, Spam)
assert_type(expr, type[Spam[Eggs]])
"#,
);

django_testcase!(
    test_generic_queryset_in_result_matching,
    r#"
from typing import TypeVar, assert_type

from django.contrib.auth.models import User
from django.db.models import QuerySet, Model

ModelType = TypeVar("ModelType", bound=Model, covariant=True)

def filter_query(query: QuerySet[ModelType]) -> QuerySet[ModelType]: ...

def use(qs: QuerySet[User]) -> None:
    expr = filter_query(qs)
    assert_type(expr, QuerySet[User, User])
"#,
);

django_testcase!(
    test_queryset_methods_return_type,
    r#"
from typing import Self, assert_type
from django.db import models

class FooQuerySet(models.QuerySet):
    def filter_bar(self):
        expr = self.filter(name='bar')
        assert_type(expr, Self)
"#,
);

django_testcase!(
    test_queryset_typing,
    r#"
from typing import assert_type
from django.db import models

class Owner(models.Model):
    name = models.CharField(max_length=30)
    age = models.IntegerField()
    cars: models.QuerySet["Car"]

class Car(models.Model):
    manufacturer = models.CharField(max_length=30)
    model = models.CharField(max_length=30)
    year = models.IntegerField()
    owner = models.ForeignKey(Owner, on_delete=models.CASCADE)

def example_1():
    owner = Owner()
    cars = owner.cars.all()
    for expr in cars:
        assert_type(expr, Car)
"#,
);

django_testcase!(
    test_foreign_key_resolved_from_string_literal_reference,
    r#"
from typing import assert_type
from django.db import models

AUTH_USER_MODEL = "User"

class User(models.Model): ...

class Profile(models.Model):
    user = models.ForeignKey(AUTH_USER_MODEL, on_delete=models.CASCADE)

expr = Profile().user
assert_type(expr, User)
"#,
);

django_testcase!(
    test_foreign_key_resolved_from_string_literal_reference_in_the_library,
    r#"
from typing import assert_type
from django.db import models
from django.conf import settings
from django.contrib.auth.models import User

class Profile(models.Model):
    user = models.ForeignKey(settings.AUTH_USER_MODEL, on_delete=models.CASCADE)

expr = Profile().user
assert_type(expr, User)
"#,
);

django_testcase!(
    test_foreign_key_reversed_type_is_typed_related_manager,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager

class Folder(models.Model):
    pass

class File(models.Model):
    folder = models.ForeignKey(Folder, on_delete=models.CASCADE)

expr = Folder.objects.get().file_set
assert_type(expr, RelatedManager[File])
"#,
);

django_testcase!(
    test_many_to_many_field_reversed_type_is_typed_many_related_manager,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager

class Tag(models.Model):
    pass

class File(models.Model):
    folder = models.ManyToManyField(Tag)

expr = Tag.objects.get().file_set
assert_type(expr, ManyRelatedManager[File])
"#,
);

// AUTH_USER_MODEL is unset, so `request.user` should be the default `User | AnonymousUser`.
fn env_user_model_default() -> TestEnv {
    let mut env = django_env().with_framework_option("django", "settings-module", "app.settings");
    env.add(
        "app.settings",
        r#"
INSTALLED_APPS = (
    'users.apps.UsersConfig',
)
# AUTH_USER_MODEL = 'users.User'
"#,
    );
    env.add(
        "users.models",
        r#"
from django.db import models
"#,
    );
    env.add(
        "users.apps",
        r#"
from django.apps import AppConfig

class UsersConfig(AppConfig):
    default_auto_field = 'django.db.models.BigAutoField'
    name = 'users'
"#,
    );
    env
}

testcase!(
    test_user_model_type_is_default,
    env_user_model_default(),
    r#"
from typing import assert_type, override
from django import views
from django.contrib.auth.models import User, AnonymousUser

class ApiView(views.View):
    @override
    def dispatch(self, request, *args, **kwargs):
        expr = request.user
        assert_type(expr, User | AnonymousUser)
        return super().dispatch(request, *args, **kwargs)
"#,
);

// AUTH_USER_MODEL points at a custom model, so `request.user` should be `CustomUser | AnonymousUser`.
fn env_user_model_from_settings() -> TestEnv {
    let mut env = django_env().with_framework_option("django", "settings-module", "app.settings");
    env.add(
        "app.settings",
        r#"
INSTALLED_APPS = (
    'users.apps.UsersConfig',
)
AUTH_USER_MODEL = 'users.CustomUser'
"#,
    );
    env.add(
        "users.models",
        r#"
from django.db import models

class CustomUser(models.Model):
    pass
"#,
    );
    env.add(
        "users.apps",
        r#"
from django.apps import AppConfig

class UsersConfig(AppConfig):
    default_auto_field = 'django.db.models.BigAutoField'
    name = 'users'
"#,
    );
    env
}

testcase!(
    test_user_model_type_parsed_from_settings,
    env_user_model_from_settings(),
    r#"
from typing import assert_type, override
from django import views
from django.contrib.auth.models import AnonymousUser
from users.models import CustomUser

class ApiView(views.View):
    @override
    def dispatch(self, request, *args, **kwargs):
        expr = request.user
        assert_type(expr, CustomUser | AnonymousUser)
        return super().dispatch(request, *args, **kwargs)
"#,
);

// A local class named `_User` must not be replaced by the user-model hint.
django_testcase!(
    test_user_model_doesnt_affect_other_symbols,
    r#"
from typing import assert_type

class _User: ...

expr = _User()
assert_type(expr, _User)
"#,
);
