//! Credential-bearing operations stay in Rust while hooks see only payload().
use super::*;
use herta_db::{
    CollectionDef, DbSession, RecordManager, RuleContext, transaction::TransactionOwner,
};

pub struct PreparedAuth {
    service: AuthService,
    kind: Kind,
    name: &'static str,
}
enum Kind {
    Registration {
        definition: CollectionDef,
        id: String,
        email: String,
        password_hash: String,
        token_key: String,
        profile: Map<String, Value>,
    },
    Tokens {
        user: AuthUser,
        token_key: String,
        family: Option<String>,
    },
}
impl PreparedAuth {
    pub(super) fn registration(
        service: AuthService,
        definition: CollectionDef,
        id: String,
        email: String,
        password_hash: String,
        token_key: String,
        profile: Map<String, Value>,
    ) -> Self {
        Self {
            service,
            name: "auth.register",
            kind: Kind::Registration {
                definition,
                id,
                email,
                password_hash,
                token_key,
                profile,
            },
        }
    }
    pub(super) fn tokens(
        service: AuthService,
        name: &'static str,
        user: AuthUser,
        token_key: String,
        family: Option<String>,
    ) -> Self {
        Self {
            service,
            name,
            kind: Kind::Tokens {
                user,
                token_key,
                family,
            },
        }
    }
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn payload(&self) -> Value {
        match &self.kind {
            Kind::Registration {
                definition,
                id,
                profile,
                ..
            } => json!({"collection":definition,"id":id,"profile":profile}),
            Kind::Tokens { user, .. } => {
                json!({"collection":{"name":user.collection},"account":user})
            }
        }
    }
    pub fn principal(&self) -> Option<String> {
        match &self.kind {
            Kind::Tokens { user, .. } => Some(user.id.clone()),
            _ => None,
        }
    }

    /// Default non-extension entrypoint uses the identical atomic completion.
    pub async fn commit(self) -> HbResult<AuthResponse> {
        let owner = TransactionOwner::new(self.service.db.clone(), self.principal());
        let receipt = owner.begin().await?;
        let profile = self.payload()["profile"].clone();
        let result = owner
            .execute(Some(receipt.id.clone()), true, move |session| {
                Box::pin(async move {
                    self.execute(session, profile, &RuleContext::default())
                        .await
                })
            })
            .await;
        match result {
            Ok(response) => {
                owner.commit(&receipt.id).await?;
                Ok(response)
            }
            Err(error) => {
                owner.finish().await?;
                Err(error)
            }
        }
    }

    /// Called exactly once inside the root owner. Credentials and issued tokens
    /// never go through the JS bridge, including on errors or afterCommit.
    pub async fn execute(
        &self,
        session: DbSession<'_>,
        profile: Value,
        context: &RuleContext,
    ) -> HbResult<AuthResponse> {
        match &self.kind {
            Kind::Registration {
                definition,
                id,
                email,
                password_hash,
                token_key,
                ..
            } => {
                let definition = SchemaManager::new(session)
                    .get_collection(&definition.name)
                    .await?;
                if definition.collection_type != CollectionType::Auth {
                    return Err(HbError::validation(
                        "authentication requires an auth collection",
                    ));
                }
                let mut profile = profile;
                let object = profile
                    .as_object_mut()
                    .ok_or_else(|| HbError::validation("profile must be an object"))?;
                // The record wrapper carries the Rust-allocated identity.
                object.remove("id");
                if object
                    .keys()
                    .any(|field| AUTH_PROTECTED_FIELDS.contains(&field.as_str()))
                {
                    return Err(HbError::validation(
                        "Auth profile contains protected fields",
                    ));
                }
                for field in definition
                    .fields
                    .iter()
                    .filter(|field| field.field_type == FieldType::File)
                {
                    if object.get(&field.name).is_some_and(|value| {
                        !value.is_null() && !value.as_array().is_some_and(Vec::is_empty)
                    }) {
                        return Err(HbError::UnsupportedMediaType(
                            "registration files require multipart".into(),
                        ));
                    }
                }
                validate_record(&definition, &mut profile, true)?;
                for field in definition
                    .fields
                    .iter()
                    .filter(|field| field.field_type == FieldType::Relation)
                {
                    if let Some(value) = profile.get(&field.name).filter(|value| !value.is_null()) {
                        let target = field
                            .options
                            .as_ref()
                            .and_then(|options| options["collection"].as_str())
                            .ok_or(HbError::Internal)?;
                        for id in value.as_str().into_iter().chain(
                            value
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_str),
                        ) {
                            RecordManager::new(session)
                                .get_authorized(target, id, None, context)
                                .await?;
                        }
                    }
                }
                let object = profile.as_object_mut().ok_or(HbError::Internal)?;
                object.insert("email".into(), email.clone().into());
                object.insert("password_hash".into(), password_hash.clone().into());
                object.insert("token_key".into(), token_key.clone().into());
                object.insert("verified".into(), false.into());
                object.insert("role".into(), "user".into());
                object.insert("failed_attempts".into(), 0.into());
                let table = quote_table(&definition.name)?;
                let mut response = session
                    .query(format!(
                        "CREATE ONLY type::record('{table}', $id) CONTENT $data RETURN AFTER"
                    ))
                    .bind(("id", id.clone()))
                    .bind((
                        "data",
                        herta_db::record::native_record_value(&definition, &profile)?,
                    ))
                    .await
                    .map_err(database_error)?
                    .check()
                    .map_err(database_error)?;
                let created: Option<Value> = response.take(0).map_err(database_error)?;
                let user =
                    user_from_value(&definition.name, false, created.ok_or(HbError::Internal)?)?;
                self.service
                    .issue_pair(session, &user, token_key, None)
                    .await
            }
            Kind::Tokens {
                user,
                token_key,
                family,
            } => {
                let mut response = session
                    .query("SELECT * FROM ONLY type::record($table, $id)")
                    .bind(("table", user.collection.clone()))
                    .bind((
                        "id",
                        user.id
                            .split_once(':')
                            .ok_or(HbError::Internal)?
                            .1
                            .to_owned(),
                    ))
                    .await
                    .map_err(database_error)?
                    .check()
                    .map_err(database_error)?;
                let account: Option<Value> = response.take(0).map_err(database_error)?;
                let account = account.ok_or(HbError::Unauthorized)?;
                if account["token_key"].as_str() != Some(token_key)
                    || !account["deleted_at"].is_null()
                {
                    return Err(HbError::Unauthorized);
                }
                if account["locked_until"]
                    .as_u64()
                    .is_some_and(|until| until > now())
                {
                    return Err(HbError::AccountLocked);
                }
                // Fence token issuance against concurrent account revocation,
                // deletion or role changes even on snapshot-isolated backends.
                // Reassigning token_key to itself is optimized away by SurrealDB;
                // a distinct internal revision makes this an actual conflicting write.
                session
                    .query(
                        "UPDATE ONLY type::record($table, $id) SET _hb_auth_issuance = $revision",
                    )
                    .bind(("table", user.collection.clone()))
                    .bind((
                        "id",
                        user.id
                            .split_once(':')
                            .ok_or(HbError::Internal)?
                            .1
                            .to_owned(),
                    ))
                    .bind(("revision", Uuid::now_v7().to_string()))
                    .await
                    .map_err(database_error)?
                    .check()
                    .map_err(database_error)?;
                let user = user_from_value(&user.collection, user.admin, account)?;
                self.service
                    .issue_pair(session, &user, token_key, family.clone())
                    .await
            }
        }
    }
}
