mod notifications;
mod pushers;
mod pushers_set;
mod pushrules;
mod pushrules_global;
mod pushrules_rule;
mod pushrules_rule_actions;
mod pushrules_rule_enabled;

pub(crate) use self::{
	notifications::get_notifications_route,
	pushers::get_pushers_route,
	pushers_set::set_pushers_route,
	pushrules::get_pushrules_all_route,
	pushrules_global::get_pushrules_global_route,
	pushrules_rule::{delete_pushrule_route, get_pushrule_route, set_pushrule_route},
	pushrules_rule_actions::{get_pushrule_actions_route, set_pushrule_actions_route},
	pushrules_rule_enabled::{get_pushrule_enabled_route, set_pushrule_enabled_route},
};
