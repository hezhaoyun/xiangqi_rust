//! The GUI for the Xiangqi engine, built with Iced.
//!
//! This file follows the Elm architecture, a Model-View-Update pattern:
//! - `XiangqiApp` is the Model: It holds the entire state of the application.
//! - `Message` is the Update trigger: It defines all possible events that can change the state.
//! - `update` is the Update logic: It processes messages to transition the state.
//! - `view` is the View: It renders the UI based on the current state.

use iced::{
    advanced::subscription::Recipe,
    executor, mouse,
    widget::{
        canvas::{self, event, Frame, Geometry, Path, Program, Stroke},
        text, Button, Column, Container, Row, TextInput,
    },
    Application, Command, Element, Font, Length, Padding, Pixels, Point, Rectangle, Renderer,
    Settings, Size, Subscription, Theme,
};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, ChildStdin, Command as StdCommand, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use engine::{
    bitboard::Board,
    constants::{Piece, Player},
    r#move::Move,
};
use futures::{channel::mpsc, stream::BoxStream};

// --- Constants ---

const CHINESE_FONT: Font = Font::with_name("PingFang SC");

// Board dimensions
const BOARD_SIZE: f32 = 500.0;
const SQUARE_SIZE: f32 = BOARD_SIZE / 9.0;
const BOARD_HEIGHT: f32 = SQUARE_SIZE * 10.0;

// Game and UCI constants
const INITIAL_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
const UCI_ENGINE_PATH: &str = "./target/release/uci";
const UCI_CMD_UCI: &str = "uci";
const UCI_CMD_ISREADY: &str = "isready";
const UCI_CMD_POSITION_FEN: &str = "position fen";
const UCI_CMD_GO_MOVETIME: &str = "go movetime 5000"; // 3 seconds
const UCI_RESPONSE_UCIOK: &str = "uciok";
const UCI_RESPONSE_READYOK: &str = "readyok";
const UCI_RESPONSE_BESTMOVE: &str = "bestmove";

// UI text constants
const STATUS_PLAYER_TURN: &str = "Your Turn";
const STATUS_ENGINE_THINKING: &str = "Engine is thinking...";
const MSG_STALEMATE: &str = "Stalemate!";

/// Runs the GUI application.
pub fn run() -> iced::Result {
    XiangqiApp::run(Settings {
        window: iced::window::Settings {
            size: Size::new(560.0, 780.0),
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
    /// Result of a player's move attempt. Contains the move, captured piece, new FEN, and optional game over message.
    PlayerMoveFinalized(Result<(Move, Piece, String, Option<String>), ()>),
}

/// The main application state (the "Model").
struct XiangqiApp {
    board: Arc<Mutex<Board>>,
    uci_engine: Child,
    uci_stdin: Arc<Mutex<ChildStdin>>,
    uci_stdout: Arc<Mutex<BufReader<ChildStdout>>>,

    // --- UI-specific state ---
    selected_square: Option<usize>,
    last_move: Option<Move>,
    move_history: Vec<(Move, Piece)>,
    fen_input: String,
    game_state: GameState,
    game_id: u64,
    board_cache: canvas::Cache,
}

/// Represents the current high-level state of the game.
#[derive(Debug, PartialEq)]
enum GameState {
    PlayerTurn,
    EngineThinking,
    GameOver(String),
}

// --- Application Setup & Lifecycle ---

impl Application for XiangqiApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    /// Called once to create the initial application state.
    fn new(_flags: ()) -> (Self, Command<Message>) {
        let (child, stdin, stdout) = Self::init_uci_engine();

        let app = XiangqiApp {
            board: Arc::new(Mutex::new(Board::from_fen(INITIAL_FEN))),
            uci_engine: child,
            uci_stdin: Arc::new(Mutex::new(stdin)),
            uci_stdout: Arc::new(Mutex::new(stdout)),
            selected_square: None,
            last_move: None,
            move_history: Vec::new(),
            fen_input: INITIAL_FEN.to_string(),
            game_state: GameState::PlayerTurn,
            game_id: 0,
            board_cache: canvas::Cache::new(),
        };
        (app, Command::none())
    }

    fn title(&self) -> String {
        String::from("Xiangqi")
    }

    /// The main update loop, dispatching messages based on the current game state.
    fn update(&mut self, message: Message) -> Command<Message> {
        match self.game_state {
            GameState::PlayerTurn => self.handle_player_turn(message),
            GameState::EngineThinking => self.handle_engine_thinking(message),
            GameState::GameOver(_) => self.handle_game_over(message),
        }
    }

    /// Subscribes to UCI engine output.
    fn subscription(&self) -> Subscription<Message> {
        Subscription::from_recipe(UciSubscription {
            uci_stdout: self.uci_stdout.clone(),
            game_id: self.game_id,
        })
    }

    /// Renders the UI based on the current state.
    fn view(&'_ self) -> Element<'_, Message> {
        let status_text = match &self.game_state {
            GameState::PlayerTurn => STATUS_PLAYER_TURN,
            GameState::EngineThinking => STATUS_ENGINE_THINKING,
            GameState::GameOver(msg) => msg.as_str(),
        };

        let canvas = canvas::Canvas::new(BoardCanvas::new(
            self.board.clone(),
            self.selected_square,
            self.last_move,
        ))
        .width(Length::Fixed(BOARD_SIZE))
        .height(Length::Fixed(BOARD_HEIGHT));

        let controls = Row::new()
            .spacing(10)
            .push(Button::new(text("New Game")).on_press(Message::NewGame))
            .push(Button::new(text("Undo Move")).on_press(Message::UndoMove));

        let fen_controls = Row::new()
            .spacing(10)
            .padding(Padding {
                top: 0.0,
                right: 30.0,
                bottom: 0.0,
                left: 30.0,
            })
            .align_items(iced::Alignment::Center)
            .push(
                TextInput::new("FEN string...", &self.fen_input)
                    .on_input(Message::FenInputChanged)
                    .width(Length::Fill),
            )
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

// --- UCI Communication ---

/// A subscription that listens for messages from the UCI engine's stdout.
struct UciSubscription {
    uci_stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    game_id: u64,
}

impl Recipe for UciSubscription {
    type Output = Message;

    fn hash(&self, state: &mut iced::advanced::Hasher) {
        use std::hash::Hash;
        struct UciListener;
        std::any::TypeId::of::<UciListener>().hash(state);
        self.game_id.hash(state);
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
                let read_result = uci_stdout.lock().unwrap().read_line(&mut line);

                match read_result {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if !line.trim().is_empty() {
                            if tx.unbounded_send(Message::UciResponse(line)).is_err() {
                                break; // Receiver dropped
                            }
                        }
                    }
                    Err(_) => break, // IO error
                }
            }
        });

        Box::pin(rx)
    }
}

// --- Update Logic Implementation ---

impl XiangqiApp {
    /// Spawns and initializes the UCI engine process.
    fn init_uci_engine() -> (Child, ChildStdin, BufReader<ChildStdout>) {
        let mut child = StdCommand::new(UCI_ENGINE_PATH)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn UCI engine");

        let stdin = child.stdin.take().expect("Failed to open stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("Failed to open stdout"));

        // Perform the UCI handshake
        writeln!(&stdin, "{}", UCI_CMD_UCI).expect("Failed to write to UCI stdin");
        Self::wait_for_uci_response(&mut stdout, UCI_RESPONSE_UCIOK);

        writeln!(&stdin, "{}", UCI_CMD_ISREADY).expect("Failed to write to UCI stdin");
        Self::wait_for_uci_response(&mut stdout, UCI_RESPONSE_READYOK);

        (child, stdin, stdout)
    }

    /// Helper to wait for a specific response from the UCI engine.
    fn wait_for_uci_response(stdout: &mut BufReader<ChildStdout>, expected: &str) {
        let mut line = String::new();
        loop {
            line.clear();
            stdout
                .read_line(&mut line)
                .expect("Failed to read from UCI stdout");
            if line.trim() == expected {
                break;
            }
        }
    }

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
                    self.apply_player_move(mv, captured, fen, game_over_state)
                }
                Err(()) => Command::none(), // Invalid move, do nothing.
            },
            Message::LoadFen => self.handle_load_fen(),
            _ => Command::none(), // Ignore other messages
        }
    }

    /// Handles all messages received while the engine is thinking.
    fn handle_engine_thinking(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::UciResponse(response) => {
                if response.starts_with(UCI_RESPONSE_BESTMOVE) {
                    self.apply_engine_move(&response)
                } else {
                    // Other UCI messages could be logged here for debugging
                    Command::none()
                }
            }
            _ => Command::none(), // Ignore other messages
        }
    }

    /// Handles all messages received after the game has ended.
    fn handle_game_over(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::NewGame => self.handle_new_game(),
            _ => Command::none(), // Ignore other messages
        }
    }

    /// Logic for when a square is clicked by the player.
    fn handle_square_clicked(&mut self, sq: usize) -> Command<Message> {
        if let Some(from_sq) = self.selected_square {
            // This is the second square click (the destination).
            self.selected_square = None;
            self.board_cache.clear();

            Command::perform(
                validate_and_perform_player_move(self.board.clone(), from_sq, sq),
                Message::PlayerMoveFinalized,
            )
        } else {
            // This is the first square click. Select it if it's a valid piece.
            let board = self.board.lock().unwrap();
            if let Some(player) = board.board[sq].player() {
                if player == board.player_to_move {
                    self.selected_square = Some(sq);
                    self.board_cache.clear(); // Redraw to show selection highlight.
                }
            }
            Command::none()
        }
    }

    /// Applies the player's validated move to the board state.
    fn apply_player_move(
        &mut self,
        mv: Move,
        captured: Piece,
        fen: String,
        game_over_state: Option<String>,
    ) -> Command<Message> {
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

    /// Parses the "bestmove" response from the engine and applies it.
    fn apply_engine_move(&mut self, response: &str) -> Command<Message> {
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

                if let Some(msg) = check_game_over_state(&mut board) {
                    self.game_state = GameState::GameOver(msg);
                } else {
                    self.game_state = GameState::PlayerTurn;
                }
            }
        }
        Command::none()
    }

    /// Triggers the UCI engine to search for and make a move.
    fn trigger_engine_move(&mut self) -> Command<Message> {
        self.game_state = GameState::EngineThinking;
        let board_fen = self.board.lock().unwrap().to_fen();
        let uci_stdin = self.uci_stdin.clone();

        Command::perform(
            async move {
                let mut uci_stdin = uci_stdin.lock().unwrap();
                writeln!(uci_stdin, "{} {}", UCI_CMD_POSITION_FEN, board_fen).ok();
                writeln!(uci_stdin, "{}", UCI_CMD_GO_MOVETIME).ok();
            },
            |_| Message::UciResponse("".to_string()), // Response is handled by the UciSubscription
        )
    }

    /// Resets the application to the initial state for a new game.
    fn handle_new_game(&mut self) -> Command<Message> {
        // Kill the old engine
        if let Err(e) = self.uci_engine.kill() {
            eprintln!("Failed to kill UCI engine: {}", e);
        }

        // Start a new one
        let (new_child, new_stdin, new_stdout) = Self::init_uci_engine();

        // Reset the state
        self.board = Arc::new(Mutex::new(Board::from_fen(INITIAL_FEN)));
        self.uci_engine = new_child;
        self.uci_stdin = Arc::new(Mutex::new(new_stdin));
        self.uci_stdout = Arc::new(Mutex::new(new_stdout));
        self.selected_square = None;
        self.last_move = None;
        self.move_history.clear();
        self.fen_input = INITIAL_FEN.to_string();
        self.game_state = GameState::PlayerTurn;
        self.game_id += 1;
        self.board_cache.clear();

        Command::none()
    }

    /// Undoes the last full turn (player and engine).
    fn handle_undo_move(&mut self) -> Command<Message> {
        if self.move_history.len() >= 2 {
            let board_lock = self.board.clone();
            let mut board = board_lock.lock().unwrap();

            // Un-do engine move
            if let Some((mv, captured)) = self.move_history.pop() {
                board.unmove_piece(mv, captured);
            }
            // Un-do player move
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
        // Use catch_unwind to prevent a panic from a malformed FEN string from crashing the app.
        if let Ok(board) = std::panic::catch_unwind(|| Board::from_fen(&self.fen_input)) {
            self.board = Arc::new(Mutex::new(board));
            self.selected_square = None;
            self.last_move = None;
            self.move_history.clear();
            self.game_state = GameState::PlayerTurn;
            self.board_cache.clear();
        }
        Command::none()
    }

    /// Parses a move in UCI format (e.g., "a0a1") into a `Move` object.
    fn parse_uci_move(&self, board: &Board, move_str: &str) -> Option<Move> {
        if move_str.len() < 4 {
            return None;
        }
        let from_file = move_str.chars().next()? as u8 - b'a';
        let from_rank = move_str.chars().nth(1)? as u8 - b'0';
        let to_file = move_str.chars().nth(2)? as u8 - b'a';
        let to_rank = move_str.chars().nth(3)? as u8 - b'0';

        let from_sq = (9 - from_rank) as usize * 9 + from_file as usize;
        let to_sq = (9 - to_rank) as usize * 9 + to_file as usize;

        let captured_piece = board.board[to_sq];
        let is_capture = captured_piece != Piece::Empty;

        Some(Move::new(
            from_sq,
            to_sq,
            if is_capture {
                Some(captured_piece)
            } else {
                None
            },
        ))
    }
}

// --- Background Tasks ---

/// Validates a player's move in a background task to avoid blocking the UI thread.
use engine::movelist::MoveList;

// ... (rest of the file is the same until validate_and_perform_player_move)

async fn validate_and_perform_player_move(
    board: Arc<Mutex<Board>>,
    from_sq: usize,
    to_sq: usize,
) -> Result<(Move, Piece, String, Option<String>), ()> {
    let mut board = board.lock().unwrap();
    let mut legal_moves = MoveList::new();
    board.generate_legal_moves(&mut legal_moves);

    if let Some(&mv) = legal_moves
        .as_slice()
        .iter()
        .find(|m| m.from_sq() == from_sq && m.to_sq() == to_sq)
    {
        let captured = board.move_piece(mv);
        let fen = board.to_fen();
        let game_over_state = check_game_over_state(&mut board);
        Ok((mv, captured, fen, game_over_state))
    } else {
        Err(()) // Invalid move
    }
}

/// Checks if the current board state is a game-over state (checkmate or stalemate).
fn check_game_over_state(board: &mut Board) -> Option<String> {
    let mut legal_moves = MoveList::new();
    board.generate_legal_moves(&mut legal_moves);
    if legal_moves.is_empty() {
        if engine::move_generator::is_king_in_check(board, board.player_to_move) {
            Some(format!(
                "{:?} wins by checkmate!",
                board.player_to_move.opponent()
            ))
        } else {
            Some(MSG_STALEMATE.to_string())
        }
    } else {
        None
    }
}

// --- Canvas Drawing Logic ---

struct BoardCanvas {
    board: Arc<Mutex<Board>>,
    selected_square: Option<usize>,
    last_move: Option<Move>,
}

impl BoardCanvas {
    fn new(
        board: Arc<Mutex<Board>>,
        selected_square: Option<usize>,
        last_move: Option<Move>,
    ) -> Self {
        Self {
            board,
            selected_square,
            last_move,
        }
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
        self.draw_pieces(&mut frame, &board);
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
    /// Draws the board grid, palaces, and river.
    fn draw_grid(&self, frame: &mut Frame) {
        // Board background
        let background = Path::rectangle(Point::new(0.0, 0.0), frame.size());
        frame.fill(&background, iced::Color::from_rgb8(235, 209, 166));

        // Horizontal lines
        for i in 0..=9 {
            let y = i as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
            let path = Path::line(
                Point::new(SQUARE_SIZE / 2.0, y),
                Point::new(BOARD_SIZE - SQUARE_SIZE / 2.0, y),
            );
            frame.stroke(&path, Stroke::default().with_width(1.0));
        }

        // Vertical lines (with river gap)
        for i in 0..=8 {
            let x = i as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
            let (y1, y2) = (SQUARE_SIZE / 2.0, BOARD_HEIGHT - SQUARE_SIZE / 2.0);
            if i == 0 || i == 8 {
                frame.stroke(
                    &Path::line(Point::new(x, y1), Point::new(x, y2)),
                    Stroke::default().with_width(1.0),
                );
            } else {
                frame.stroke(
                    &Path::line(Point::new(x, y1), Point::new(x, 4.5 * SQUARE_SIZE)),
                    Stroke::default().with_width(1.0),
                );
                frame.stroke(
                    &Path::line(Point::new(x, 5.5 * SQUARE_SIZE), Point::new(x, y2)),
                    Stroke::default().with_width(1.0),
                );
            }
        }

        // River text
        self.draw_river_text(frame, "漢界", 2.0 * SQUARE_SIZE, 5.0 * SQUARE_SIZE);
        self.draw_river_text(frame, "楚河", 7.0 * SQUARE_SIZE, 5.0 * SQUARE_SIZE);

        // Palace diagonal lines
        self.draw_palace_diagonal(frame, 3.5, 0.5, 5.5, 2.5);
        self.draw_palace_diagonal(frame, 3.5, 2.5, 5.5, 0.5);
        self.draw_palace_diagonal(frame, 3.5, 7.5, 5.5, 9.5);
        self.draw_palace_diagonal(frame, 3.5, 9.5, 5.5, 7.5);
    }

    /// Draws the text for the river.
    fn draw_river_text(&self, frame: &mut Frame, text: &str, x: f32, y: f32) {
        frame.fill_text(canvas::Text {
            content: text.to_string(),
            position: Point::new(x, y),
            color: iced::Color::from_rgb8(100, 100, 100),
            size: Pixels(SQUARE_SIZE * 0.6),
            font: CHINESE_FONT,
            horizontal_alignment: iced::alignment::Horizontal::Center,
            vertical_alignment: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
    }

    /// Draws a diagonal line in a palace, with coordinates in square units.
    fn draw_palace_diagonal(&self, frame: &mut Frame, x1_sq: f32, y1_sq: f32, x2_sq: f32, y2_sq: f32) {
        let path = Path::line(
            Point::new(x1_sq * SQUARE_SIZE, y1_sq * SQUARE_SIZE),
            Point::new(x2_sq * SQUARE_SIZE, y2_sq * SQUARE_SIZE),
        );
        frame.stroke(&path, Stroke::default().with_width(1.0));
    }

    /// Draws highlights for the last move made.
    fn draw_highlights(&self, frame: &mut Frame) {
        if let Some(mv) = self.last_move {
            self.highlight_square(
                frame,
                mv.from_sq(),
                iced::Color::from_rgba(1.0, 1.0, 0.0, 0.3),
            );
            self.highlight_square(
                frame,
                mv.to_sq(),
                iced::Color::from_rgba(0.0, 1.0, 0.0, 0.3),
            );
        }
    }

    /// Draws all the pieces on the board.
    fn draw_pieces(&self, frame: &mut Frame, board: &Board) {
        for (i, &piece) in board.board.iter().enumerate() {
            if piece != Piece::Empty {
                let r = i / 9;
                let c = i % 9;
                self.draw_single_piece(frame, piece, r, c);
            }
        }
    }

    /// Draws a single chess piece.
    fn draw_single_piece(&self, frame: &mut Frame, piece: Piece, r: usize, c: usize) {
        let x = c as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
        let y = r as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;

        let color = if piece.player() == Some(Player::Red) {
            iced::Color::from_rgb8(255, 0, 0)
        } else {
            iced::Color::from_rgb8(0, 0, 0)
        };

        // Draw piece shadow
        let shadow_offset = 3.0;
        let shadow_circle =
            Path::circle(Point::new(x + shadow_offset, y + shadow_offset), SQUARE_SIZE * 0.4);
        frame.fill(&shadow_circle, iced::Color::from_rgba8(0, 0, 0, 0.4));

        // Draw piece background circle
        let circle = Path::circle(Point::new(x, y), SQUARE_SIZE * 0.4);
        frame.fill(&circle, iced::Color::from_rgb8(240, 240, 240));
        frame.stroke(
            &circle,
            Stroke::default()
                .with_width(2.0)
                .with_color(iced::Color::from_rgb8(0, 0, 0)),
        );

        // Draw piece character
        frame.fill_text(canvas::Text {
            content: get_chinese_piece_char(piece).to_string(),
            position: Point::new(x, y),
            color,
            size: Pixels(SQUARE_SIZE * 0.6),
            font: CHINESE_FONT,
            horizontal_alignment: iced::alignment::Horizontal::Center,
            vertical_alignment: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
    }

    /// Draws a highlight border around the currently selected square.
    fn draw_selected_square_highlight(&self, frame: &mut Frame) {
        if let Some(sq) = self.selected_square {
            let r = sq / 9;
            let c = sq % 9;
            let x = c as f32 * SQUARE_SIZE;
            let y = r as f32 * SQUARE_SIZE;
            let path = Path::rectangle(Point::new(x, y), Size::new(SQUARE_SIZE, SQUARE_SIZE));
            frame.stroke(
                &path,
                Stroke::default()
                    .with_width(3.0)
                    .with_color(iced::Color::from_rgb(0.0, 1.0, 0.0)),
            );
        }
    }

    /// Fills a square with a given color.
    fn highlight_square(&self, frame: &mut Frame, sq: usize, color: iced::Color) {
        let r = sq / 9;
        let c = sq % 9;
        let x = c as f32 * SQUARE_SIZE;
        let y = r as f32 * SQUARE_SIZE;
        let path = Path::rectangle(Point::new(x, y), Size::new(SQUARE_SIZE, SQUARE_SIZE));
        frame.fill(&path, color);
    }
}

// --- Utility Functions ---

/// Maps a `Piece` enum to its corresponding Chinese character representation.
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
