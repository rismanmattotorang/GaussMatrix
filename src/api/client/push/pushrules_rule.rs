use axum::extract::State;
use ruma::{
	api::client::push::{delete_pushrule, get_pushrule, set_pushrule},
	events::{GlobalAccountDataEventType, push_rules::PushRulesEvent},
	push::{
		InsertPushRuleError, PredefinedContentRuleId, PredefinedOverrideRuleId,
		RemovePushRuleError,
	},
};
use gaussmatrix_core::{Err, Result, err};

use crate::Ruma;

/// # `GET /_matrix/client/r0/pushrules/{scope}/{kind}/{ruleId}`
///
/// Retrieves a single specified push rule for this user.
pub(crate) async fn get_pushrule_route(
	State(services): State<crate::State>,
	body: Ruma<get_pushrule::v3::Request>,
) -> Result<get_pushrule::v3::Response> {
	let sender_user = body
		.sender_user
		.as_ref()
		.expect("user is authenticated");

	// remove old deprecated mentions push rules as per MSC4210
	#[expect(deprecated)]
	if body.rule_id.as_str() == PredefinedContentRuleId::ContainsUserName.as_str()
		|| body.rule_id.as_str() == PredefinedOverrideRuleId::ContainsDisplayName.as_str()
		|| body.rule_id.as_str() == PredefinedOverrideRuleId::RoomNotif.as_str()
	{
		return Err!(Request(NotFound("Push rule not found.")));
	}

	let event: PushRulesEvent = services
		.account_data
		.get_global(sender_user, GlobalAccountDataEventType::PushRules)
		.await
		.map_err(|_| err!(Request(NotFound("PushRules event not found."))))?;

	let rule = event
		.content
		.global
		.get(body.kind.clone(), &body.rule_id)
		.map(Into::into);

	if let Some(rule) = rule {
		Ok(get_pushrule::v3::Response { rule })
	} else {
		Err!(Request(NotFound("Push rule not found.")))
	}
}

/// # `PUT /_matrix/client/r0/pushrules/global/{kind}/{ruleId}`
///
/// Creates a single specified push rule for this user.
pub(crate) async fn set_pushrule_route(
	State(services): State<crate::State>,
	body: Ruma<set_pushrule::v3::Request>,
) -> Result<set_pushrule::v3::Response> {
	let sender_user = body.sender_user();
	let mut account_data: PushRulesEvent = services
		.account_data
		.get_global(sender_user, GlobalAccountDataEventType::PushRules)
		.await
		.map_err(|_| err!(Request(NotFound("PushRules event not found."))))?;

	if let Err(error) = account_data.content.global.insert(
		body.rule.clone(),
		body.after.as_deref(),
		body.before.as_deref(),
	) {
		use InsertPushRuleError::*;

		return match error {
			| ServerDefaultRuleId => Err!(Request(InvalidParam(
				"Rule IDs starting with a dot are reserved for server-default rules."
			))),
			| RelativeToServerDefaultRule => Err!(Request(InvalidParam(
				"Can't place a push rule relatively to a server-default rule."
			))),
			| BeforeHigherThanAfter => Err!(Request(InvalidParam(
				"The before rule has a higher priority than the after rule."
			))),
			| InvalidRuleId =>
				Err!(Request(InvalidParam("Rule ID containing invalid characters."))),

			| UnknownRuleId =>
				Err!(Request(NotFound("The before or after rule could not be found."))),

			| _ => Err!(Request(InvalidParam("Invalid data."))),
		};
	}

	let ty = GlobalAccountDataEventType::PushRules;
	services
		.account_data
		.update(None, sender_user, ty.to_string().into(), &serde_json::to_value(account_data)?)
		.await?;

	Ok(set_pushrule::v3::Response {})
}

/// # `DELETE /_matrix/client/r0/pushrules/global/{kind}/{ruleId}`
///
/// Deletes a single specified push rule for this user.
pub(crate) async fn delete_pushrule_route(
	State(services): State<crate::State>,
	body: Ruma<delete_pushrule::v3::Request>,
) -> Result<delete_pushrule::v3::Response> {
	let sender_user = body.sender_user();

	let mut account_data: PushRulesEvent = services
		.account_data
		.get_global(sender_user, GlobalAccountDataEventType::PushRules)
		.await
		.map_err(|_| err!(Request(NotFound("PushRules event not found."))))?;

	if let Err(error) = account_data
		.content
		.global
		.remove(body.kind.clone(), &body.rule_id)
	{
		return match error {
			| RemovePushRuleError::ServerDefault =>
				Err!(Request(InvalidParam("Cannot delete a server-default pushrule."))),

			| RemovePushRuleError::NotFound => Err!(Request(NotFound("Push rule not found."))),

			| _ => Err!(Request(InvalidParam("Invalid data."))),
		};
	}

	let ty = GlobalAccountDataEventType::PushRules;
	services
		.account_data
		.update(None, sender_user, ty.to_string().into(), &serde_json::to_value(account_data)?)
		.await?;

	Ok(delete_pushrule::v3::Response {})
}
