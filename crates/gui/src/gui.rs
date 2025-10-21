//! The GUI for the Xiangqi engine, built with Iced.
//!
//! This file follows the Elm architecture, a Model-View-Update pattern:
//! - `XiangqiApp` is the Model: It holds the entire state of the application.
//! - `Message` is the Update trigger: It defines all possible events that can change the state.
//! - `update` is the Update logic: It processes messages to transition the state.
//! - `view` is the View: It renders the UI based on the current state.

use iced::{
    executor,
    widget::{canvas::{self, event, Frame, Geometry, Path, Program, Stroke}, text, Button, Column, Container, Row, TextInput},
    Application, Command, Element, Font, Length, Padding, Pixels, Point, Rectangle, Renderer, Settings, Size, Subscription, Theme, mouse
};
use std::sync::{Arc, Mutex};
use engine::{
    bitboard::Board,
    constants::{Piece, Player},
    r#move::Move,
};
use std::process::{Command as StdCommand, Stdio, ChildStdin, ChildStdout};
use std::io::{BufRead, BufReader, Write};
use iced::advanced::subscription::{Recipe};
use futures::stream::BoxStream;
use std::thread;
use futures::channel::mpsc;

const CHINESE_FONT: Font = Font::with_name("PingFang SC");

const BOARD_SIZE: f32 = 500.0;
const SQUARE_SIZE: f32 = BOARD_SIZE / 9.0;
const BOARD_HEIGHT: f32 = SQUARE_SIZE * 10.0;

/// Runs the GUI application.
pub fn run() -> iced::Result {
    XiangqiApp::run(Settings {
        window: iced::window::Settings {
            size: Size::new(700.0, 800.0),
            ..iced::window::Settings::default()
        },
        ..Settings::default()
    })
}

/// Defines the messages that can be sent to the `update` function.
#[derive(Debug, Clone)]
enum Message {
    NewGame,
    UndoMove,
    SquareClicked(usize),
    UciResponse(String),
    FenInputChanged(String),
    LoadFen,
    PlayerMoveFinalized(Result<(Move, Piece, String, Option<String>), ()>),
}

/// The main application state (the "Model").
struct XiangqiApp {
    board: Arc<Mutex<Board>>,
    uci_stdin: Arc<Mutex<ChildStdin>>,
    uci_stdout: Arc<Mutex<BufReader<ChildStdout>>>,

    // --- UI-specific state ---
    selected_square: Option<usize>,
    last_move: Option<Move>,
    move_history: Vec<(Move, Piece)>,
    fen_input: String,
    game_state: GameState,
    board_cache: canvas::Cache,
}


/// Represents the current high-level state of the game.
enum GameState {
    PlayerTurn,
    EngineThinking,
    GameOver(String),
}

// --- Application Logic ---

impl Application for XiangqiApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let initial_fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        let mut child = StdCommand::new("./target/release/uci")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn UCI engine");

        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("Failed to open stdout"));

        // UCI Initialization
        writeln!(stdin, "uci").expect("Failed to write to UCI stdin");
        let mut line = String::new();
        loop {
            line.clear();
            stdout.read_line(&mut line).expect("Failed to read from UCI stdout");
            if line.trim() == "uciok" {
                break;
            }
        }
        writeln!(stdin, "isready").expect("Failed to write to UCI stdin");
        loop {
            line.clear();
            stdout.read_line(&mut line).expect("Failed to read from UCI stdout");
            if line.trim() == "readyok" {
                break;
            }
        }

        let app = XiangqiApp {
            board: Arc::new(Mutex::new(Board::from_fen(initial_fen))),
            uci_stdin: Arc::new(Mutex::new(stdin)),
            uci_stdout: Arc::new(Mutex::new(stdout)),
            selected_square: None,
            last_move: None,
            move_history: Vec::new(),
            fen_input: initial_fen.to_string(),
            game_state: GameState::PlayerTurn,
            board_cache: canvas::Cache::new(),
        };
        (app, Command::none())
    }

    fn title(&self) -> String {
        String::from("Xiangqi")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match self.game_state {
            GameState::PlayerTurn => self.handle_player_turn(message),
            GameState::EngineThinking => self.handle_engine_thinking(message),
            GameState::GameOver(_) => self.handle_game_over(message),
        }
    }
    
    fn subscription(&self) -> Subscription<Message> {
        Subscription::from_recipe(UciSubscription {
            uci_stdout: self.uci_stdout.clone(),
        })
    }

    fn view(&'_ self) -> Element<'_, Message> {
        let status_text = match &self.game_state {
            GameState::PlayerTurn => "Your Turn",
            GameState::EngineThinking => "Engine is thinking...",
            GameState::GameOver(ref msg) => msg.as_str(),
        };

        let canvas = canvas::Canvas::new(BoardCanvas::new(self.board.clone(), self.selected_square, self.last_move))
            .width(Length::Fixed(BOARD_SIZE))
            .height(Length::Fixed(BOARD_HEIGHT));

        let controls = Row::new()
            .spacing(10)
            .push(Button::new(text("New Game")).on_press(Message::NewGame))
            .push(Button::new(text("Undo Move")).on_press(Message::UndoMove));

        let fen_controls = Row::new()
            .spacing(10)
            .padding(Padding {top: 0.0, right: 100.0, bottom: 0.0, left: 100.0})
            .align_items(iced::Alignment::Center)
            .push(TextInput::new("FEN string...", &self.fen_input).on_input(Message::FenInputChanged).width(Length::Fill))
            .push(Button::new(text("Load FEN")).on_press(Message::LoadFen));

        let content = Column::new()
            .spacing(20)
            .align_items(iced::Alignment::Center)
            .push(text(status_text).size(Pixels(24.0)))
            .push(canvas)
            .push(controls)
            .push(fen_controls);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

struct UciSubscription {
    uci_stdout: Arc<Mutex<BufReader<ChildStdout>>>,
}

impl Recipe for UciSubscription {
    type Output = Message;

    fn hash(&self, state: &mut iced::advanced::Hasher) {
        use std::hash::Hash;
        struct UciListener;
        std::any::TypeId::of::<UciListener>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: BoxStream<'static, (iced::Event, iced::widget::canvas::event::Status)>,
    ) -> BoxStream<'static, Self::Output> {
        let (tx, rx) = mpsc::unbounded();

        let uci_stdout = self.uci_stdout;
        thread::spawn(move || {
            loop {
                let mut line = String::new();
                let read_result = {
                    let mut stdout_guard = uci_stdout.lock().unwrap();
                    stdout_guard.read_line(&mut line)
                };

                match read_result {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if !line.trim().is_empty() {
                            if tx.unbounded_send(Message::UciResponse(line)).is_err() {
                                break; // Receiver dropped
                            }
                        }
                    }
                    Err(_) => break, // Error
                }
            }
        });

        Box::pin(rx)
    }
}

// --- Update Helper Functions ---

impl XiangqiApp {
    /// Handles all messages received when it is the player's turn.
    fn handle_player_turn(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SquareClicked(sq) => self.handle_square_clicked(sq),
            Message::NewGame => self.handle_new_game(),
            Message::UndoMove => self.handle_undo_move(),
            Message::FenInputChanged(new_fen) => {
                self.fen_input = new_fen;
                Command::none()
            }
            Message::PlayerMoveFinalized(result) => match result {
                Ok((mv, captured, fen, game_over_state)) => {
                    self.fen_input = fen;
                    self.move_history.push((mv, captured));
                    self.last_move = Some(mv);
                    self.board_cache.clear();

                    if let Some(msg) = game_over_state {
                        self.game_state = GameState::GameOver(msg);
                        Command::none()
                    } else {
                        self.trigger_engine_move()
                    }
                }
                Err(()) => Command::none(), // Invalid move
            },
            Message::LoadFen => self.handle_load_fen(),
            // Ignore other messages like EngineMoved
            _ => Command::none(),
        }
    }

    /// Handles all messages received while the engine is thinking.
    fn handle_engine_thinking(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::UciResponse(response) => {
                if response.starts_with("bestmove") {
                    let parts: Vec<&str> = response.split_whitespace().collect();
                    if let Some(move_str) = parts.get(1) {
                        let board_lock = self.board.clone();
                        let mut board = board_lock.lock().unwrap();
                        if let Some(mv) = self.parse_uci_move(&board, move_str) {
                            let captured = board.move_piece(mv);
                            self.fen_input = board.to_fen();
                            self.move_history.push((mv, captured));
                            self.last_move = Some(mv);
                            self.board_cache.clear();

                            let legal_moves = board.generate_legal_moves();
                            if legal_moves.is_empty() {
                                if engine::move_gen::is_king_in_check(&board, board.player_to_move) {
                                    self.game_state = GameState::GameOver(format!("{:?} wins by checkmate!", board.player_to_move.opponent()));
                                } else {
                                    self.game_state = GameState::GameOver("Stalemate!".to_string());
                                }
                            } else {
                                self.game_state = GameState::PlayerTurn;
                            }
                        }
                    }
                }
                Command::none()
            }
            // Ignore other messages
            _ => Command::none(),
        }
    }


    /// Handles all messages received after the game has ended.
    fn handle_game_over(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::NewGame => self.handle_new_game(),
            // Ignore other messages
            _ => Command::none(),
        }
    }

    /// Logic for when a square is clicked.
    fn handle_square_clicked(&mut self, sq: usize) -> Command<Message> {
        if let Some(from_sq) = self.selected_square {
            // Second square clicked, clear selection for UI responsiveness.
            self.selected_square = None;
            self.board_cache.clear();
            let board = self.board.clone();

            return Command::perform(
                async move {
                    let mut board = board.lock().unwrap();
                    // Perform heavy computation in background
                    let legal_moves = board.generate_legal_moves();
                    if let Some(&mv) = legal_moves.iter().find(|m| m.from_sq() == from_sq && m.to_sq() == sq) {
                        // Valid move
                        let captured = board.move_piece(mv);
                        let fen = board.to_fen();

                        let next_legal_moves = board.generate_legal_moves();
                        let game_over_state = if next_legal_moves.is_empty() {
                            if engine::move_gen::is_king_in_check(&board, board.player_to_move) {
                                Some(format!("{:?} wins by checkmate!", board.player_to_move.opponent()))
                            } else {
                                Some("Stalemate!".to_string())
                            }
                        } else {
                            None
                        };
                        Ok((mv, captured, fen, game_over_state))
                    } else {
                        // Invalid move
                        Err(())
                    }
                },
                Message::PlayerMoveFinalized,
            );
        } else {
            // First square selected.
            self.selected_square = Some(sq);
            self.board_cache.clear(); // Redraw to show selection
        }
        Command::none()
    }

    /// Triggers the engine to search for and make a move.
    fn trigger_engine_move(&mut self) -> Command<Message> {
        self.game_state = GameState::EngineThinking;
        let board_fen = self.board.lock().unwrap().to_fen();
        let uci_stdin = self.uci_stdin.clone();

        Command::perform(
            async move {
                let mut uci_stdin = uci_stdin.lock().unwrap();
                writeln!(uci_stdin, "position fen {}", board_fen).ok();
                writeln!(uci_stdin, "go movetime 3000").ok();
            },
            |_| Message::UciResponse("".to_string()), // We get the response via subscription
        )
    }


    /// Resets the application to the initial state for a new game.
    fn handle_new_game(&mut self) -> Command<Message> {
        let initial_fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        self.board = Arc::new(Mutex::new(Board::from_fen(initial_fen)));
        self.selected_square = None;
        self.last_move = None;
        self.move_history.clear();
        self.fen_input = initial_fen.to_string();
        self.game_state = GameState::PlayerTurn;
        self.board_cache.clear();
        Command::none()
    }

    /// Undoes the last full turn (player and engine).
    fn handle_undo_move(&mut self) -> Command<Message> {
        if self.move_history.len() >= 2 {
            let board_lock = self.board.clone();
            let mut board = board_lock.lock().unwrap();

            if let Some((mv, captured)) = self.move_history.pop() {
                board.unmove_piece(mv, captured);
            }
            if let Some((mv, captured)) = self.move_history.pop() {
                board.unmove_piece(mv, captured);
            }

            self.fen_input = board.to_fen();
            self.game_state = GameState::PlayerTurn;
            self.last_move = self.move_history.last().map(|(mv, _)| *mv);
            self.selected_square = None;
            self.board_cache.clear();
        }
        Command::none()
    }

    /// Loads a new board state from the FEN string in the input box.
    fn handle_load_fen(&mut self) -> Command<Message> {
        let new_board = std::panic::catch_unwind(|| Board::from_fen(&self.fen_input)).ok();

        if let Some(board) = new_board {
            self.board = Arc::new(Mutex::new(board));
            self.selected_square = None;
            self.last_move = None;
            self.move_history.clear();
            self.game_state = GameState::PlayerTurn;
            self.board_cache.clear();
        } 
        Command::none()
    }



    fn parse_uci_move(&self, board: &Board, move_str: &str) -> Option<Move> {
        if move_str.len() < 4 {
            return None;
        }
        let from_file = move_str.chars().nth(0)? as u8 - b'a';
        let from_rank = move_str.chars().nth(1)? as u8 - b'0';
        let to_file = move_str.chars().nth(2)? as u8 - b'a';
        let to_rank = move_str.chars().nth(3)? as u8 - b'0';

        let from_sq = (9 - from_rank) as usize * 9 + from_file as usize;
        let to_sq = (9 - to_rank) as usize * 9 + to_file as usize;

        let captured_piece = board.board[to_sq];

        Some(Move::new(from_sq, to_sq, if captured_piece == engine::constants::Piece::Empty { None } else { Some(captured_piece) }))
    }
}

// --- Canvas Drawing Logic ---

struct BoardCanvas {
    board: Arc<Mutex<Board>>,
    selected_square: Option<usize>,
    last_move: Option<Move>,
}

impl BoardCanvas {
    fn new(board: Arc<Mutex<Board>>, selected_square: Option<usize>, last_move: Option<Move>) -> Self {
        Self { board, selected_square, last_move }
    }
}

impl Program<Message> for BoardCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let board = self.board.lock().unwrap();
        let mut frame = Frame::new(renderer, bounds.size());

        self.draw_grid(&mut frame);
        self.draw_highlights(&mut frame);
        self.draw_pieces(&board, &mut frame);
        self.draw_selected_square_highlight(&mut frame);

        vec![frame.into_geometry()]
    }
    
    fn update(
        &self,
        _state: &mut Self::State,
        event: event::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        if let event::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                let c = (pos.x / SQUARE_SIZE).floor() as usize;
                let r = (pos.y / SQUARE_SIZE).floor() as usize;
                if r < 10 && c < 9 {
                    let sq = r * 9 + c;
                    return (event::Status::Captured, Some(Message::SquareClicked(sq)));
                }
            }
        }
        (event::Status::Ignored, None)
    }
}

// --- Canvas Drawing Helper Functions ---

impl BoardCanvas {
    fn draw_grid(&self, frame: &mut Frame) {
        // Board background
        let background = Path::rectangle(Point::new(0.0, 0.0), frame.size());
        frame.fill(&background, iced::Color::from_rgb8(235, 209, 166));

        // Horizontal lines
        for i in 0..=9 {
            let y = i as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
            let path = Path::line(Point::new(SQUARE_SIZE/2.0, y), Point::new(BOARD_SIZE - SQUARE_SIZE/2.0, y));
            frame.stroke(&path, Stroke::default().with_width(1.0));
        }

        // Vertical lines (with river gap)
        for i in 0..=8 {
            let x = i as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
            if i == 0 || i == 8 {
                // Outer border lines are continuous
                let path = Path::line(Point::new(x, SQUARE_SIZE/2.0), Point::new(x, BOARD_HEIGHT - SQUARE_SIZE/2.0));
                frame.stroke(&path, Stroke::default().with_width(1.0));
            } else {
                // Inner lines have a gap for the river
                let path1 = Path::line(Point::new(x, SQUARE_SIZE/2.0), Point::new(x, 4.5 * SQUARE_SIZE));
                let path2 = Path::line(Point::new(x, 5.5 * SQUARE_SIZE), Point::new(x, BOARD_HEIGHT - SQUARE_SIZE/2.0));
                frame.stroke(&path1, Stroke::default().with_width(1.0));
                frame.stroke(&path2, Stroke::default().with_width(1.0));
            }
        }

        // River text
        let river_text = |frame: &mut Frame, text: &str, x: f32, y: f32| {
            let text_widget = canvas::Text {
                content: text.to_string(),
                position: Point::new(x, y),
                color: iced::Color::from_rgb8(0, 0, 0),
                size: Pixels(SQUARE_SIZE * 0.8),
                font: CHINESE_FONT,
                horizontal_alignment: iced::alignment::Horizontal::Center,
                vertical_alignment: iced::alignment::Vertical::Center,
                line_height: iced::widget::text::LineHeight::default(),
                shaping: iced::widget::text::Shaping::Basic,
            };
            frame.fill_text(text_widget);
        };

        // Place "楚河" and "漢界"
        river_text(frame, "漢界", 2.0 * SQUARE_SIZE, 5.4 * SQUARE_SIZE - SQUARE_SIZE * 0.4);
        river_text(frame, "楚河", 7.0 * SQUARE_SIZE, 5.4 * SQUARE_SIZE - SQUARE_SIZE * 0.4);

        // Palace diagonal lines
        let palace_path = |frame: &mut Frame, x1, y1, x2, y2| {
            let path = Path::line(Point::new(x1,y1), Point::new(x2,y2));
            frame.stroke(&path, Stroke::default().with_width(1.0));
        };
        palace_path(frame, 3.5*SQUARE_SIZE, 0.5*SQUARE_SIZE, 5.5*SQUARE_SIZE, 2.5*SQUARE_SIZE);
        palace_path(frame, 3.5*SQUARE_SIZE, 2.5*SQUARE_SIZE, 5.5*SQUARE_SIZE, 0.5*SQUARE_SIZE);
        palace_path(frame, 3.5*SQUARE_SIZE, 7.5*SQUARE_SIZE, 5.5*SQUARE_SIZE, 9.5*SQUARE_SIZE);
        palace_path(frame, 3.5*SQUARE_SIZE, 9.5*SQUARE_SIZE, 5.5*SQUARE_SIZE, 7.5*SQUARE_SIZE);
    }

    fn draw_highlights(&self, frame: &mut Frame) {
        // Highlight last move
        if let Some(mv) = self.last_move {
            let from_sq = mv.from_sq();
            let to_sq = mv.to_sq();

            let r_from = from_sq / 9;
            let c_from = from_sq % 9;
            let x_from = c_from as f32 * SQUARE_SIZE;
            let y_from = r_from as f32 * SQUARE_SIZE;
            let from_path = Path::rectangle(Point::new(x_from, y_from), Size::new(SQUARE_SIZE, SQUARE_SIZE));
            frame.fill(&from_path, iced::Color::from_rgba(1.0, 1.0, 0.0, 0.3));

            let r_to = to_sq / 9;
            let c_to = to_sq % 9;
            let x_to = c_to as f32 * SQUARE_SIZE;
            let y_to = r_to as f32 * SQUARE_SIZE;
            let to_path = Path::rectangle(Point::new(x_to, y_to), Size::new(SQUARE_SIZE, SQUARE_SIZE));
            frame.fill(&to_path, iced::Color::from_rgba(0.0, 1.0, 0.0, 0.3));
        }
    }

    fn draw_pieces(&self, board: &Board, frame: &mut Frame) {
        for r in 0..10 {
            for c in 0..9 {
                let piece = board.board[r * 9 + c];
                if piece != Piece::Empty {
                    let x = c as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
                    let y = r as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
                    let color = if piece.player() == Some(Player::Red) {
                        iced::Color::from_rgb8(255, 0, 0)
                    } else {
                        iced::Color::from_rgb8(0, 0, 0)
                    };
                    let text_content = get_chinese_piece_char(piece).to_string();
                    let text = canvas::Text {
                        content: text_content,
                        position: Point::new(x, y),
                        color,
                        size: Pixels(SQUARE_SIZE * 0.6),
                        font: CHINESE_FONT,
                        horizontal_alignment: iced::alignment::Horizontal::Center,
                        vertical_alignment: iced::alignment::Vertical::Center,
                        line_height: iced::widget::text::LineHeight::default(),
                        shaping: iced::widget::text::Shaping::Basic,
                    };
                    let circle = Path::circle(Point::new(x, y), SQUARE_SIZE * 0.4);
                    let shadow_offset = 3.0;
                    let shadow_circle = Path::circle(Point::new(x + shadow_offset, y + shadow_offset), SQUARE_SIZE * 0.4);
                    frame.fill(&shadow_circle, iced::Color::from_rgba8(0, 0, 0, 0.4));
                    frame.fill(&circle, iced::Color::from_rgb8(240, 240, 240));
                    frame.stroke(&circle, Stroke::default().with_width(2.0).with_color(iced::Color::from_rgb8(0, 0, 0)));
                    frame.fill_text(text);
                }
            }
        }
    }

    fn draw_selected_square_highlight(&self, frame: &mut Frame) {
        if let Some(sq) = self.selected_square {
            let r = sq / 9;
            let c = sq % 9;
            let x = c as f32 * SQUARE_SIZE;
            let y = r as f32 * SQUARE_SIZE;
            let path = Path::rectangle(Point::new(x,y), Size::new(SQUARE_SIZE, SQUARE_SIZE));
            frame.stroke(&path, Stroke::default().with_width(3.0).with_color(iced::Color::from_rgb(0.0, 1.0, 0.0)));
        }
    }
}

// --- Utility Functions ---

fn get_chinese_piece_char(piece: Piece) -> char {
    match piece {
        Piece::BKing => '将',
        Piece::BGuard => '士',
        Piece::BBishop => '象',
        Piece::BHorse => '馬',
        Piece::BRook => '車',
        Piece::BCannon => '砲',
        Piece::BPawn => '卒',
        Piece::Empty => '·',
        Piece::RKing => '帅',
        Piece::RGuard => '仕',
        Piece::RBishop => '相',
        Piece::RHorse => '傌',
        Piece::RRook => '俥',
        Piece::RCannon => '炮',
        Piece::RPawn => '兵',
    }
}
