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
    EnterControlMode,
    EnterResizeMode,
    Move(Direction),
    AddPane,
    AddColumn,
    ResizeColumn(bool),
    ResetColumnWidth,
    ToggleZoom,
    ScrollPane(Direction),
    ReorderPane(Direction),
    CyclePresentation,
    CycleLayout,
    SaveSession,
    Send(Vec<u8>),
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlAction {
    Cancel,
    EnterResizeMode,
    ToggleZoom,
    AddPane,
    AddColumn,
    Move(Direction),
    MovePane(Direction),
    MoveColumn(Direction),
    ResetSpace,
    CycleLayout,
    CyclePresentation,
    SaveSession,
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResizeAction {
    Cancel,
    ResizePane(bool),
    ResizeColumn(bool),
    ResetSpace,
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
            KeyCode::Char('z') => return Action::ToggleZoom,
            KeyCode::Char('p') => return Action::EnterControlMode,
            KeyCode::Char('r') => return Action::EnterResizeMode,
            KeyCode::Char('n') => return Action::AddPane,
            KeyCode::Char('c') => return Action::AddColumn,
            KeyCode::Char('s') => return Action::SaveSession,
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
            return direction
                .map(|direction| match direction {
                    Direction::Left | Direction::Right => Action::ScrollPane(direction),
                    Direction::Up | Direction::Down => Action::ReorderPane(direction),
                })
                .unwrap_or(Action::Ignore);
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

pub fn map_control_key(event: KeyEvent) -> ControlAction {
    let shifted = event.modifiers.contains(KeyModifiers::SHIFT);
    if shifted {
        let direction = match event.code {
            KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Left => Some(Direction::Left),
            KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Right => Some(Direction::Right),
            KeyCode::Char('k') | KeyCode::Char('K') | KeyCode::Up => Some(Direction::Up),
            KeyCode::Char('j') | KeyCode::Char('J') | KeyCode::Down => Some(Direction::Down),
            _ => None,
        };
        if let Some(direction) = direction {
            return match direction {
                Direction::Left | Direction::Right => ControlAction::MovePane(direction),
                Direction::Up | Direction::Down => ControlAction::Ignore,
            };
        }
    }

    match event.code {
        KeyCode::Esc | KeyCode::Char('p') => ControlAction::Cancel,
        KeyCode::Char('r') => ControlAction::EnterResizeMode,
        KeyCode::Char('z') => ControlAction::ToggleZoom,
        KeyCode::Char('n') => ControlAction::AddPane,
        KeyCode::Char('c') => ControlAction::AddColumn,
        KeyCode::Char('h') | KeyCode::Left => ControlAction::Move(Direction::Left),
        KeyCode::Char('l') | KeyCode::Right => ControlAction::Move(Direction::Right),
        KeyCode::Char('k') | KeyCode::Up => ControlAction::Move(Direction::Up),
        KeyCode::Char('j') | KeyCode::Down => ControlAction::Move(Direction::Down),
        KeyCode::Char('[') | KeyCode::Char(',') => ControlAction::MovePane(Direction::Left),
        KeyCode::Char(']') | KeyCode::Char('.') => ControlAction::MovePane(Direction::Right),
        KeyCode::Char('{') => ControlAction::MoveColumn(Direction::Left),
        KeyCode::Char('}') => ControlAction::MoveColumn(Direction::Right),
        KeyCode::Char('0') | KeyCode::Char('b') => ControlAction::ResetSpace,
        KeyCode::Char('m') => ControlAction::CycleLayout,
        KeyCode::Char('w') => ControlAction::CyclePresentation,
        KeyCode::Char('s') => ControlAction::SaveSession,
        _ => ControlAction::Ignore,
    }
}

pub fn map_resize_key(event: KeyEvent) -> ResizeAction {
    match event.code {
        KeyCode::Esc | KeyCode::Char('r') | KeyCode::Char('p') => ResizeAction::Cancel,
        KeyCode::Char('j') | KeyCode::Char('=') | KeyCode::Char('+') | KeyCode::Down => {
            ResizeAction::ResizePane(true)
        }
        KeyCode::Char('k') | KeyCode::Char('-') | KeyCode::Up => ResizeAction::ResizePane(false),
        KeyCode::Char('l') | KeyCode::Right => ResizeAction::ResizeColumn(true),
        KeyCode::Char('h') | KeyCode::Left => ResizeAction::ResizeColumn(false),
        KeyCode::Char('0') | KeyCode::Char('b') => ResizeAction::ResetSpace,
        _ => ResizeAction::Ignore,
    }
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
            map_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT)),
            Action::ToggleZoom
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT)),
            Action::EnterControlMode
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT)),
            Action::EnterResizeMode
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT)),
            Action::AddPane
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::ALT)),
            Action::AddColumn
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT)),
            Action::SaveSession
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT | KeyModifiers::SHIFT)),
            Action::ReorderPane(Direction::Down)
        );
    }

    #[test]
    fn maps_control_mode_shortcuts() {
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE)),
            ControlAction::ToggleZoom
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
            ControlAction::Move(Direction::Down)
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)),
            ControlAction::Move(Direction::Up)
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
            ControlAction::EnterResizeMode
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
            ControlAction::SaveSession
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
            ControlAction::AddPane
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
            ControlAction::AddColumn
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE)),
            ControlAction::MovePane(Direction::Right)
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SHIFT)),
            ControlAction::MovePane(Direction::Right)
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Char('}'), KeyModifiers::NONE)),
            ControlAction::MoveColumn(Direction::Right)
        );
        assert_eq!(
            map_control_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ControlAction::Cancel
        );
    }

    #[test]
    fn maps_resize_mode_shortcuts() {
        assert_eq!(
            map_resize_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
            ResizeAction::ResizePane(true)
        );
        assert_eq!(
            map_resize_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)),
            ResizeAction::ResizePane(false)
        );
        assert_eq!(
            map_resize_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)),
            ResizeAction::ResizeColumn(true)
        );
        assert_eq!(
            map_resize_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)),
            ResizeAction::ResizeColumn(false)
        );
        assert_eq!(
            map_resize_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE)),
            ResizeAction::ResetSpace
        );
        assert_eq!(
            map_resize_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ResizeAction::Cancel
        );
    }
}
