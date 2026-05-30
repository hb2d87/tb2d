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
    Send(Vec<u8>),
    Ignore,
}

pub fn map_key(event: KeyEvent) -> Action {
    if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('q') {
        return Action::Quit;
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
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
            map_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            Action::Send(b"x".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT)),
            Action::Send(b"\x1bx".to_vec())
        );
    }
}
