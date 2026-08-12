use yozora_ast::Position;

pub fn calc_last_source_line(position: Option<&Position>) -> Option<usize> {
    let position = position?;
    let start = position.start;
    let end = position.end;
    Some(if end.column == 1 && end.line > start.line {
        end.line - 1
    } else {
        end.line
    })
}
