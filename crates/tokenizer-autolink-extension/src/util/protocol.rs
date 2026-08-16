use yozora_character::{is_alphanumeric, AsciiCodePoint, NodePoint};

use super::email::eat_extend_email_address;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExtendedProtocolEatResult {
    pub recognized: bool,
    pub valid: bool,
    pub next_index: usize,
}

pub(crate) fn eat_extended_protocol_autolink(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> ExtendedProtocolEatResult {
    let (prefix, allows_resource) = if has_prefix(node_points, start_index, end_index, "mailto:") {
        ("mailto:", false)
    } else if has_prefix(node_points, start_index, end_index, "xmpp:") {
        ("xmpp:", true)
    } else {
        return ExtendedProtocolEatResult {
            recognized: false,
            valid: false,
            next_index: start_index + 1,
        };
    };

    let address_start_index = start_index + prefix.len();
    let email = eat_extend_email_address(node_points, address_start_index, end_index);
    if !email.valid {
        return ExtendedProtocolEatResult {
            recognized: true,
            valid: false,
            next_index: email.next_index.max(address_start_index).min(end_index),
        };
    }

    let mut next_index = email.next_index;
    if allows_resource
        && next_index < end_index
        && node_points[next_index].code_point == AsciiCodePoint::SLASH as i32
    {
        let mut resource_end_index = next_index + 1;
        while resource_end_index < end_index {
            let code_point = node_points[resource_end_index].code_point;
            if !is_alphanumeric(code_point)
                && code_point != AsciiCodePoint::AT_SIGN as i32
                && code_point != AsciiCodePoint::DOT as i32
            {
                break;
            }
            resource_end_index += 1;
        }
        if resource_end_index > next_index + 1 {
            next_index = resource_end_index;
        }
    }

    ExtendedProtocolEatResult {
        recognized: true,
        valid: true,
        next_index,
    }
}

fn has_prefix(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
    prefix: &str,
) -> bool {
    start_index + prefix.len() <= end_index
        && prefix
            .bytes()
            .enumerate()
            .all(|(index, byte)| node_points[start_index + index].code_point == i32::from(byte))
}
