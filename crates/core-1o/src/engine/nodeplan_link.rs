use crate::engine::{Lexer, Node, NodeValue, Parser};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePlanWorkspaceLink {
    pub workspace_id: String,
    pub connection_kind: String,
}

pub fn parse_nodeplan_workspace_link(text: &str) -> Result<Option<NodePlanWorkspaceLink>, String> {
    let ast = Parser::new(Lexer::new(text).tokenize())
        .parse()
        .map_err(|error| format!("invalid 1o nodeplan link syntax: {error:?}"))?;
    let Some(nodeplan) = ast.iter().find(|node| node.key == "nodeplan") else {
        return Ok(None);
    };
    let NodeValue::Block(children) = &nodeplan.value else {
        return Err("nodeplan link must be a block".into());
    };
    let workspace_id = string_child(children, "workspace_id")
        .ok_or_else(|| "nodeplan.workspace_id must be a string".to_string())?;
    let connection_kind = children
        .iter()
        .find(|node| node.key == "connection")
        .and_then(|node| match &node.value {
            NodeValue::Block(children) => string_child(children, "kind"),
            _ => None,
        })
        .unwrap_or_else(|| "local_registry".into());
    if connection_kind != "local_registry" {
        return Err(format!(
            "unsupported NodePlan connection kind '{connection_kind}'; v1 supports only local_registry"
        ));
    }
    Ok(Some(NodePlanWorkspaceLink {
        workspace_id,
        connection_kind,
    }))
}

fn string_child(nodes: &[Node], key: &str) -> Option<String> {
    nodes.iter().find(|node| node.key == key).and_then(|node| {
        if let NodeValue::String(value) = &node.value {
            Some(value.clone())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodeplan_link_selects_workspace_without_granting_authority() {
        let link = parse_nodeplan_workspace_link(
            "{ nodeplan { workspace_id \"nodeplan-real-service-pilot\" connection { kind \"local_registry\" } } }",
        )
        .unwrap()
        .unwrap();
        assert_eq!(link.workspace_id, "nodeplan-real-service-pilot");
        assert_eq!(link.connection_kind, "local_registry");
    }
}
