use cae_model::{Model, Units};

fn disconnected_bars() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("a", 0.0, 0.0).expect("node a");
    model.add_node("b", 500.0, 0.0).expect("node b");
    model.add_node("c", 1000.0, 0.0).expect("node c");
    model.add_node("d", 1000.0, 500.0).expect("node d");
    model.add_element("e1", "a", "b", None).expect("element e1");
    model.add_element("e2", "c", "d", None).expect("element e2");
    model
}

#[test]
fn merge_moves_elements_loads_and_supports_to_the_surviving_node() {
    let mut model = disconnected_bars();
    model.add_load("b", Some(10.0), Some(5.0)).expect("load b");
    model.add_load("c", Some(2.0), Some(-1.0)).expect("load c");
    model.add_support("b", true, false).expect("support b");
    model.add_support("c", false, true).expect("support c");
    let revision = model.revision;

    model.merge_nodes("b", "c").expect("merge b into c");

    assert_eq!(model.revision, revision + 1);
    assert!(model.node("b").is_none(), "the merged node is removed");
    let survivor = model.node("c").expect("the target node survives");
    assert_eq!((survivor.x, survivor.y), (1000.0, 0.0), "the target keeps its coordinates");
    assert_eq!(model.elements.len(), 2);
    assert_eq!((model.elements[0].from.as_str(), model.elements[0].to.as_str()), ("a", "c"));
    assert_eq!((model.elements[1].from.as_str(), model.elements[1].to.as_str()), ("c", "d"));

    assert_eq!(model.loads.len(), 1, "loads at both nodes collapse into one entry");
    let load = &model.loads[0];
    assert_eq!(load.node, "c");
    assert_eq!((load.fx, load.fy), (12.0, 4.0));

    assert_eq!(model.supports.len(), 1);
    let support = &model.supports[0];
    assert_eq!(support.node, "c");
    assert!(support.dx && support.dy, "support flags are OR-ed");
}

#[test]
fn merge_adds_spring_stiffness_component_wise() {
    let mut model = disconnected_bars();
    model.set_spring("b", Some(10.0), Some(2.0), None, None).expect("spring b");
    model.set_spring("c", Some(5.0), None, None, None).expect("spring c");

    model.merge_nodes("b", "c").expect("merge b into c");

    let spring = model.supports[0].spring.expect("merged spring");
    assert_eq!((spring.kx, spring.ky), (15.0, 2.0));
}

#[test]
fn merge_rejects_an_element_that_would_collapse() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("a", 0.0, 0.0).expect("node a");
    model.add_node("b", 100.0, 0.0).expect("node b");
    model.add_element("e1", "a", "b", None).expect("element");
    let revision = model.revision;

    let error = model.merge_nodes("a", "b").expect_err("the element would become a self-loop");
    assert_eq!(error.code, "element/degenerate");
    assert!(error.message.contains("e1"), "message: {}", error.message);
    assert_eq!(model.revision, revision, "a rejected merge keeps the revision");
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.elements.len(), 1);
}

#[test]
fn merge_rejects_conflicting_prescribed_displacements() {
    let mut model = disconnected_bars();
    model.prescribe_displacement("b", Some(0.1), None, None, None).expect("prescribe b");
    model.prescribe_displacement("c", Some(0.2), None, None, None).expect("prescribe c");
    let revision = model.revision;

    let error = model.merge_nodes("b", "c").expect_err("different settlements cannot merge");
    assert_eq!(error.code, "support/merge-conflict");
    assert!(error.message.contains("prescribed"), "message: {}", error.message);
    assert_eq!(model.revision, revision);
    assert_eq!(model.supports.len(), 2);
}

#[test]
fn merge_rejects_an_inclined_direction_combined_with_a_rigid_support() {
    let mut model = disconnected_bars();
    model.add_support_direction("b", 1.0, 1.0).expect("inclined support b");
    model.add_support("c", true, false).expect("rigid support c");

    let error = model.merge_nodes("b", "c").expect_err("the constraint frames conflict");
    assert_eq!(error.code, "support/merge-conflict");
    assert_eq!(model.supports.len(), 2);
}

#[test]
fn merge_reports_unknown_and_identical_identifiers() {
    let mut model = disconnected_bars();
    assert_eq!(model.merge_nodes("b", "b").expect_err("same id").code, "node/merge-invalid");
    assert_eq!(model.merge_nodes("missing", "b").expect_err("unknown source").code, "node/unknown-id");
    assert_eq!(model.merge_nodes("b", "missing").expect_err("unknown target").code, "node/unknown-id");
    assert_eq!(model.nodes.len(), 4);
}
