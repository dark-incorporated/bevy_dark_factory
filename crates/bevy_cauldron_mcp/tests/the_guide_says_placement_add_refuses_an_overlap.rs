//! **The guide says `placement_add` refuses an overlap unless `replace` is true** (#443, a wish from the style study).
//!
//! The refusal and the replacing add are proved in the editor (`placement_add_refuses_an_overlap_unless_told_to_replace`); what an
//! agent reads before it calls is the guide, so the guide names the rule, the parameter and what the answer then says.

#[test]
fn the_guide_names_the_overlap_rule_and_the_parameter() {
    let guide = bevy_cauldron_mcp::docs::GUIDE;
    assert!(
        guide.contains("`placement_add`") && guide.contains("`replace: true`") && guide.contains("refuses"),
        "the guide does not say that `placement_add` refuses an overlap unless `replace: true`"
    );
    assert!(
        guide.contains("names what it replaced") && guide.contains("replacement"),
        "the guide does not say that a replacing add names what it replaced and that the hand-back card lists it as a replacement"
    );
}
