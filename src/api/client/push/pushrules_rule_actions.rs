use axum::extract::State;
use ruma::{
	api::client::push::{get_pushrule_actions, set_pushrule_actions},
	events::{GlobalAccountDataEventType, push_rules::PushRulesEvent},
	push::{PredefinedContentRuleId, PredefinedOverrideRuleId},
};
use gaussmatrix_core::{Err, Result, err};

use crate::Ruma;

/// # `GET /_matrix/client/r0/pushrules/global/{kind}/{ruleId}/actions`
///
/// Gets the actions of a single specified push rule for this user.
pub(crate) async fn get_pushrule_actions_route(
	State(services): State<crate::State>,
	body: Ruma<get_pushrule_actions::v3::Request>,
) -> Result<get_pushrule_actions::v3::Response> {
	let sender_user = body.sender_user();

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

	let actions = event
		.content
		.global
		.get(body.kind.clone(), &body.rule_id)
		.map(|rule| rule.actions().to_owned())
		.ok_or_else(|| err!(Request(NotFound("Push rule not found."))))?;

	Ok(get_pushrule_actions::v3::Response { actions })
}

/// # `PUT /_matrix/client/r0/pushrules/global/{kind}/{ruleId}/actions`
///
/// Sets the actions of a single specified push rule for this user.
pub(crate) async fn set_pushrule_actions_route(
	State(services): State<crate::State>,
	body: Ruma<set_pushrule_actions::v3::Request>,
) -> Result<set_pushrule_actions::v3::Response> {
	let sender_user = body.sender_user();

	let mut account_data: PushRulesEvent = services
		.account_data
		.get_global(sender_user, GlobalAccountDataEventType::PushRules)
		.await
		.map_err(|_| err!(Request(NotFound("PushRules event not found."))))?;

	if account_data
		.content
		.global
		.set_actions(body.kind.clone(), &body.rule_id, body.actions.clone().into())
		.is_err()
	{
		return Err!(Request(NotFound("Push rule not found.")));
	}

	let ty = GlobalAccountDataEventType::PushRules;
	services
		.account_data
		.update(None, sender_user, ty.to_string().into(), &serde_json::to_value(account_data)?)
		.await?;

	Ok(set_pushrule_actions::v3::Response {})
}
