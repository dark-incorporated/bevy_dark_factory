//! **`world.query` reaches the components an agent asks for** (#664).
//!
//! BRP's `world.query` resolves every component through the app's `AppTypeRegistry`, and
//! `bevy_remote::builtin_methods::get_component_ids` **silently skips** a component it cannot find there
//! unless the caller passes `strict`. With every named component skipped the filter is empty, the query
//! matches every entity, and the response is `[]` — which a caller reads as "this component is on
//! nothing". The running editor answered exactly that for `Node`, `Name`, `Text`, `Camera` and
//! `GlobalTransform`, while `blueprint/state` on the same socket was correct.
//!
//! So the assertion is the one the failure broke: a component an agent names is in the registry, and an
//! entity carrying it is found. A test that only asked "does the method answer" would have passed on the
//! broken build, because `[]` is an answer.
//!
//! The components are compared by **`TypeId`**, not by a spelled path: `bevy_remote` matches a request
//! against the registry by type path, and a test that hard-codes the string can be wrong about it in the
//! same way a caller can be. `TypeId::of::<T>()` is the type itself.

use bevy::prelude::*;
use std::any::TypeId;

/// **Whether the registry carries this exact type**, which is what `get_component_type_registration`
/// looks up before it hands the query a component id.
fn registered(app: &App, id: TypeId) -> bool {
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    registry
        .iter()
        .any(|registration| registration.type_id() == id)
}

/// **A component an agent names is in the registry**, which is the condition `world.query` needs.
///
/// The five here are the ones the issue measured returning `[]`. If any is absent the query skips it, and
/// a query naming only absent components matches everything and answers `[]`.
#[test]
fn every_component_an_agent_queries_is_registered() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    bevy_dark_factory::harness::register_the_components_an_agent_queries(&mut app);

    let absent: Vec<&str> = [
        ("Name", TypeId::of::<Name>()),
        ("Node", TypeId::of::<Node>()),
        ("Text", TypeId::of::<Text>()),
        ("Camera", TypeId::of::<Camera>()),
        ("GlobalTransform", TypeId::of::<GlobalTransform>()),
    ]
    .into_iter()
    .filter(|(_, id)| !registered(&app, *id))
    .map(|(name, _)| name)
    .collect();

    assert!(
        absent.is_empty(),
        "**the registry has no entry for {}**, so `world.query` skips them, the filter is empty and the \
         answer is `[]` (#664). The registration is in \
         `harness::register_the_components_an_agent_queries`, beside the transport that needs it.",
        absent.join(", ")
    );
}

/// **An entity carrying a registered component is found, which is the answer the issue asked for.**
///
/// This asserts the count rather than "not an error": the failure was a *successful* empty answer, so an
/// assertion that tolerated `0` would be the same defect in a test.
#[test]
fn an_entity_carrying_a_registered_component_is_found() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    bevy_dark_factory::harness::register_the_components_an_agent_queries(&mut app);
    app.world_mut().spawn((Name::new("a panel"), Node::default()));
    app.update();

    assert!(
        registered(&app, TypeId::of::<Node>()),
        "`Node` is registered, so a query naming it resolves"
    );

    let found = app
        .world_mut()
        .query_filtered::<Entity, With<Node>>()
        .iter(app.world())
        .count();
    assert_eq!(
        found, 1,
        "one entity carries `Node`, so a query for it must find one; `0` is the empty answer #664 \
         measured and would read as \"this component is on nothing\"."
    );
}
