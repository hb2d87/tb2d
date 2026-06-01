use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    Move(Direction),
    ResizeColumn(bool),
    ResetColumnWidth,
    ScrollPane(Direction),
    ReorderPane(Direction),
    CyclePresentation,
    CycleLayout,
    Send(Vec<u8>),
    Ignore,
}

pub fn map_key(event: KeyEvent) -> Action {
    if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('q') {
        return Action::Quit;
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        match event.code {
            KeyCode::Char('-') => return Action::ResizeColumn(false),
            KeyCode::Char('=') | KeyCode::Char('+') => return Action::ResizeColumn(true),
            KeyCode::Char('0') => return Action::ResetColumnWidth,
            KeyCode::Char('w') => return Action::CyclePresentation,
            KeyCode::Char('m') => return Action::CycleLayout,
            KeyCode::PageUp => return Action::ScrollPane(Direction::Up),
            KeyCode::PageDown => return Action::ScrollPane(Direction::Down),
            _ => {}
        }
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            let direction = match event.code {
                KeyCode::Char('h') | KeyCode::Left => Some(Direction::Left),
                KeyCode::Char('l') | KeyCode::Right => Some(Direction::Right),
                KeyCode::Char('k') | KeyCode::Up => Some(Direction::Up),
                KeyCode::Char('j') | KeyCode::Down => Some(Direction::Down),
                _ => None,
            };
            return direction.map(|direction| match direction {
                Direction::Left | Direction::Right => Action::ScrollPane(direction),
                Direction::Up | Direction::Down => Action::ReorderPane(direction),
            }).unwrap_or(Action::Ignore);
        }
        let direction = match event.code {
            KeyCode::Char('h') | KeyCode::Left => Some(Direction::Left),
            KeyCode::Char('l') | KeyCode::Right => Some(Direction::Right),
            KeyCode::Char('k') | KeyCode::Up => Some(Direction::Up),
            KeyCode::Char('j') | KeyCode::Down => Some(Direction::Down),
            _ => None,
        };
        if let Some(direction) = direction {
            return Action::Move(direction);
        }
    }
    encode_key(event).map(Action::Send).unwrap_or(Action::Ignore)
}

fn encode_key(event: KeyEvent) -> Option<Vec<u8>> {
    let bytes = match event.code {
        KeyCode::Char(character) if event.modifiers.contains(KeyModifiers::CONTROL) => {
            vec![(character.to_ascii_lowercase() as u8).saturating_sub(b'a') + 1]
        }
        KeyCode::Char(character) if event.modifiers.contains(KeyModifiers::ALT) => {
            let mut bytes = vec![0x1b];
            bytes.extend(character.to_string().into_bytes());
            bytes
        }
        KeyCode::Char(character) => character.to_string().into_bytes(),
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        _ => return None,
    };
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_navigation_and_regular_input() {
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT)),
            Action::Move(Direction::Left)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT)),
            Action::Move(Direction::Left)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT)),
            Action::Move(Direction::Right)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT)),
            Action::Move(Direction::Up)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT)),
            Action::Move(Direction::Down)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            Action::Send(b"x".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT)),
            Action::Send(b"\x1bx".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('='), KeyModifiers::ALT)),
            Action::ResizeColumn(true)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::ALT)),
            Action::ResetColumnWidth
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::ALT)),
            Action::ScrollPane(Direction::Up)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT | KeyModifiers::SHIFT)),
            Action::ReorderPane(Direction::Down)
        );
    }
}
