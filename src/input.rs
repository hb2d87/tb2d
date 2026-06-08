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

pub fn map_key(event: KeyEvent, application_cursor: bool) -> Action {
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
                    Direction::Left | Direction::Right => Action::Ignore,
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
    encode_key(event, application_cursor).map(Action::Send).unwrap_or(Action::Ignore)
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

fn encode_key(event: KeyEvent, application_cursor: bool) -> Option<Vec<u8>> {
    let cursor = |normal: &'static [u8], application: &'static [u8]| {
        if application_cursor { application } else { normal }.to_vec()
    };
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
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Left => cursor(b"\x1b[D", b"\x1bOD"),
        KeyCode::Right => cursor(b"\x1b[C", b"\x1bOC"),
        KeyCode::Up => cursor(b"\x1b[A", b"\x1bOA"),
        KeyCode::Down => cursor(b"\x1b[B", b"\x1bOB"),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(number) => function_key(number)?,
        _ => return None,
    };
    Some(bytes)
}

fn function_key(number: u8) -> Option<Vec<u8>> {
    let bytes = match number {
        1 => b"\x1bOP".as_slice(),
        2 => b"\x1bOQ",
        3 => b"\x1bOR",
        4 => b"\x1bOS",
        5 => b"\x1b[15~",
        6 => b"\x1b[17~",
        7 => b"\x1b[18~",
        8 => b"\x1b[19~",
        9 => b"\x1b[20~",
        10 => b"\x1b[21~",
        11 => b"\x1b[23~",
        12 => b"\x1b[24~",
        _ => return None,
    };
    Some(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_navigation_and_regular_input() {
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT), false),
            Action::Move(Direction::Left)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT), false),
            Action::Move(Direction::Left)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT), false),
            Action::Move(Direction::Right)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT), false),
            Action::Move(Direction::Up)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT), false),
            Action::Move(Direction::Down)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), false),
            Action::Send(b"x".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT), false),
            Action::Send(b"\x1bx".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('='), KeyModifiers::ALT), false),
            Action::ResizeColumn(true)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::ALT), false),
            Action::ResetColumnWidth
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::ALT), false),
            Action::ScrollPane(Direction::Up)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT), false),
            Action::ToggleZoom
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT), false),
            Action::EnterControlMode
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT), false),
            Action::EnterResizeMode
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT), false),
            Action::AddPane
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::ALT), false),
            Action::AddColumn
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT), false),
            Action::SaveSession
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT | KeyModifiers::SHIFT), false),
            Action::ReorderPane(Direction::Down)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT | KeyModifiers::SHIFT), false),
            Action::Ignore
        );
    }

    #[test]
    fn maps_plain_arrows_for_normal_and_application_cursor_modes() {
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), false),
            Action::Send(b"\x1b[B".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), true),
            Action::Send(b"\x1bOB".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), true),
            Action::Send(b"\x1bOA".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), true),
            Action::Send(b"\x1bOC".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), true),
            Action::Send(b"\x1bOD".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), false),
            Action::Send(b"\x1b[5~".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), false),
            Action::Send(b"\x1b[6~".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Insert, KeyModifiers::NONE), false),
            Action::Send(b"\x1b[2~".to_vec())
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE), false),
            Action::Send(b"\x1b[15~".to_vec())
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
