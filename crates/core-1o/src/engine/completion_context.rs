// engine/completion_context.rs

use crate::engine::{Node, NodeValue};

#[derive(Debug)]
pub enum CompletionKind {
    Key,
    Value(String), // key name
    None,
}

#[derive(Debug)]
pub struct CompletionContext<'a> {
    pub parent_stack: Vec<&'a Node>,
    pub kind: CompletionKind,
}

pub fn compute_completion_context<'a>(
    ast: &'a [Node],
    cursor: usize,
) -> CompletionContext<'a> {
    let mut stack = vec![];

    // walk recursively and find deepest node containing cursor
    fn walk<'a>(nodes: &'a [Node], cursor: usize, stack: &mut Vec<&'a Node>) {
        for n in nodes {
            if cursor >= n.span.start && cursor <= n.span.end {
                stack.push(n);

                if let NodeValue::Block(children) = &n.value {
                    walk(children, cursor, stack);
                }
            }
        }
    }

    walk(ast, cursor, &mut stack);

    // determine if we're typing inside a key or value
    // (we can refine this later with token-level info)
    let kind = if stack.last().is_none() {
        CompletionKind::Key
    } else {
        let node = stack.last().unwrap();
        match &node.value {
            NodeValue::Block(_) => CompletionKind::Key,
            _ => CompletionKind::Value(node.key.clone()),
        }
    };

    CompletionContext {
        parent_stack: stack,
        kind,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Lexer, Parser};

    #[test]
    fn inter_property_whitespace_requests_parent_keys_not_previous_value() {
        let text = "{\n  page {\n    id \"home\"\n    route \"/\"\n    title \"Hello\"\n  }\n}\n";
        let ast = Parser::new(Lexer::new(text).tokenize()).parse().unwrap();
        // This is line 3, character 3, matching the real LSP smoke test.
        let cursor = text.lines().take(3).map(|line| line.len() + 1).sum::<usize>() + 3;
        let context = compute_completion_context(&ast, cursor);
        let stack = context.parent_stack.iter().map(|node| node.key.as_str()).collect::<Vec<_>>();
        assert_eq!(stack, vec!["page"]);
        assert!(matches!(context.kind, CompletionKind::Key));
    }

    #[test]
    fn whitespace_between_section_blocks_selects_page_schema() {
        let text = "{ page { section { id \"first\" }\n  section { id \"second\" } } }";
        let ast = Parser::new(Lexer::new(text).tokenize()).parse().unwrap();
        let cursor = text.find("  section").unwrap() + 1;
        let context = compute_completion_context(&ast, cursor);
        assert_eq!(context.parent_stack.iter().map(|n| n.key.as_str()).collect::<Vec<_>>(), vec!["page"]);
        assert!(matches!(context.kind, CompletionKind::Key));
    }

    #[test]
    fn nested_string_value_uses_source_offsets_for_completion_context() {
        let text = "{ service { name \"type_serverd\" runtime \"runtime-type-serverd-real\" } }";
        let ast = Parser::new(Lexer::new(text).tokenize()).parse().unwrap();
        let cursor = text.find("type_serverd").unwrap() + 1;
        let context = compute_completion_context(&ast, cursor);

        assert_eq!(
            context
                .parent_stack
                .iter()
                .map(|node| node.key.as_str())
                .collect::<Vec<_>>(),
            vec!["service", "name"]
        );
        assert!(matches!(context.kind, CompletionKind::Value(ref key) if key == "name"));
    }
}
