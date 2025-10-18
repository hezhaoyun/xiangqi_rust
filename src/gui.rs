//! The GUI for the Xiangqi engine, built with Iced.

use iced::widget::{canvas, text, Button, Column, Container, Row};
use iced::{
    executor, Application, Command, Element, Length, Point, Pixels, Rectangle, Settings,
    Size, Subscription, Theme, Renderer, Font,
};
use iced::widget::canvas::{Program, Geometry, Frame, Stroke, Event as CanvasEvent};
use iced::mouse::{Cursor, Event as MouseEvent};
use iced::widget::canvas::event::Status;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

use crate::bitboard::Board;
use crate::constants::{Piece, Player};
use crate::engine::Engine;
use crate::r#move::Move;

const CHINESE_FONT: Font = Font::with_name("PingFang SC");

const BOARD_SIZE: f32 = 600.0;
const SQUARE_SIZE: f32 = BOARD_SIZE / 9.0;
const BOARD_HEIGHT: f32 = SQUARE_SIZE * 10.0;

pub fn run() -> iced::Result {
    XiangqiApp::run(Settings {
        window: iced::window::Settings {
            size: Size::new(700.0, 800.0),
            ..iced::window::Settings::default()
        },
        ..Settings::default()
    })
}

#[derive(Debug, Clone)]
enum Message {
    NewGame,
    SquareClicked(usize),
    EngineMoved(Move),
}

struct XiangqiApp {
    board: Arc<Mutex<Board>>,
    engine: Arc<TokioMutex<Engine>>,
    selected_square: Option<usize>,
    game_state: GameState,
    board_cache: canvas::Cache,
}

enum GameState {
    PlayerTurn,
    EngineThinking,
    GameOver(String),
}

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

impl Application for XiangqiApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let app = XiangqiApp {
            board: Arc::new(Mutex::new(Board::from_fen(
                "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1",
            ))),
            engine: Arc::new(TokioMutex::new(Engine::new(16))),
            selected_square: None,
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
            GameState::PlayerTurn => match message {
                Message::SquareClicked(sq) => {
                    if let Some(from_sq) = self.selected_square {
                        let board_lock = self.board.clone();
                        let mut board = board_lock.lock().unwrap();
                        let legal_moves = board.generate_legal_moves();
                        let mv = legal_moves.iter().find(|m| m.from_sq() == from_sq && m.to_sq() == sq);

                        if let Some(&mv) = mv {
                            board.move_piece(mv);
                            self.selected_square = None;
                            self.board_cache.clear();

                            let current_player = board.player_to_move.opponent(); // The player who just moved
                            let legal_moves_after_move = board.generate_legal_moves();
                            if legal_moves_after_move.is_empty() {
                                if crate::move_gen::is_king_in_check(&board, current_player) {
                                    self.game_state = GameState::GameOver(format!("{:?} wins by checkmate!", current_player));
                                } else {
                                    self.game_state = GameState::GameOver("Stalemate!".to_string());
                                }
                                return Command::none();
                            } else {
                                self.game_state = GameState::EngineThinking;
                                let mut board_clone_for_engine = board.clone(); // Clone the board
                                drop(board); // Drop the lock

                                let engine_clone = Arc::clone(&self.engine);
                                return Command::perform(
                                    async move {
                                        let mut engine = engine_clone.lock().await;
                                        let (best_move, _score) = engine.search(&mut board_clone_for_engine, 8); // Use cloned board
                                        best_move
                                    },
                                    Message::EngineMoved,
                                );
                            }
                        } else {
                            self.selected_square = Some(sq);
                        }
                    } else {
                        self.selected_square = Some(sq);
                    }
                    Command::none()
                }
                Message::NewGame => {
                    self.board = Arc::new(Mutex::new(Board::from_fen(
                        "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1",
                    )));
                    self.selected_square = None;
                    self.game_state = GameState::PlayerTurn;
                    self.board_cache.clear();
                    Command::none()
                }
                _ => Command::none(),
            },
            GameState::EngineThinking => match message {
                Message::EngineMoved(mv) => {
                    let board_lock = self.board.clone();
                    let mut board = board_lock.lock().unwrap();
                    board.move_piece(mv);
                    self.board_cache.clear();

                    let current_player = board.player_to_move.opponent(); // The player who just moved
                    let legal_moves_after_move = board.generate_legal_moves();
                    if legal_moves_after_move.is_empty() {
                        if crate::move_gen::is_king_in_check(&board, current_player) {
                            self.game_state = GameState::GameOver(format!("{:?} wins by checkmate!", current_player));
                        } else {
                            self.game_state = GameState::GameOver("Stalemate!".to_string());
                        }
                    } else {
                        self.game_state = GameState::PlayerTurn;
                    }
                    Command::none()
                }
                _ => Command::none(),
            },
            GameState::GameOver(_) => match message {
                Message::NewGame => {
                    self.board = Arc::new(Mutex::new(Board::from_fen(
                        "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1",
                    )));
                    self.selected_square = None;
                    self.game_state = GameState::PlayerTurn;
                    self.board_cache.clear();
                    Command::none()
                }
                _ => Command::none(),
            },
        }
    }
    
    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn view(&'_ self) -> Element<'_, Message> {
        let status_text = match &self.game_state {
            GameState::PlayerTurn => "Your Turn",
            GameState::EngineThinking => "Engine is thinking...",
            GameState::GameOver(ref msg) => msg.as_str(),
        };

        let canvas = canvas(BoardCanvas::new(&self.board, self.selected_square))
            .width(Length::Fixed(BOARD_SIZE))
            .height(Length::Fixed(BOARD_HEIGHT));

        let content = Column::new()
            .spacing(20)
            .align_items(iced::Alignment::Center)
            .push(text(status_text).size(Pixels(24.0)))
            .push(canvas)
            .push(
                Row::new()
                    .spacing(10)
                    .push(Button::new(text("New Game")).on_press(Message::NewGame)),
            );

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

struct BoardCanvas<'a> {
    board: &'a Mutex<Board>,
    selected_square: Option<usize>,
}

impl<'a> BoardCanvas<'a> {
    fn new(board: &'a Mutex<Board>, selected_square: Option<usize>) -> Self {
        Self { board, selected_square }
    }
}

impl<'a> Program<Message> for BoardCanvas<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let board = self.board.lock().unwrap();
        let mut frame = Frame::new(renderer, bounds.size());

        // Draw board
        let background = canvas::Path::rectangle(Point::new(0.0, 0.0), frame.size());
        frame.fill(&background, iced::Color::from_rgb8(235, 209, 166));

        // Draw lines
        for i in 0..=9 {
            let y = i as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
            let path = canvas::Path::line(Point::new(SQUARE_SIZE/2.0, y), Point::new(BOARD_SIZE - SQUARE_SIZE/2.0, y));
            frame.stroke(&path, Stroke::default().with_width(1.0));
        }
        for i in 0..=8 {
            let x = i as f32 * SQUARE_SIZE + SQUARE_SIZE / 2.0;
            let path = canvas::Path::line(Point::new(x, SQUARE_SIZE/2.0), Point::new(x, BOARD_HEIGHT - SQUARE_SIZE/2.0));
            frame.stroke(&path, Stroke::default().with_width(1.0));
        }
        
        // Draw palace lines
        let palace_path = |frame: &mut Frame, x1, y1, x2, y2| {
            let path = canvas::Path::line(Point::new(x1,y1), Point::new(x2,y2));
            frame.stroke(&path, Stroke::default().with_width(1.0));
        };
        palace_path(&mut frame, 3.5*SQUARE_SIZE, 0.5*SQUARE_SIZE, 5.5*SQUARE_SIZE, 2.5*SQUARE_SIZE);
        palace_path(&mut frame, 3.5*SQUARE_SIZE, 2.5*SQUARE_SIZE, 5.5*SQUARE_SIZE, 0.5*SQUARE_SIZE);
        palace_path(&mut frame, 3.5*SQUARE_SIZE, 7.5*SQUARE_SIZE, 5.5*SQUARE_SIZE, 9.5*SQUARE_SIZE);
        palace_path(&mut frame, 3.5*SQUARE_SIZE, 9.5*SQUARE_SIZE, 5.5*SQUARE_SIZE, 7.5*SQUARE_SIZE);


        // Draw pieces
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
                    let text = canvas::Text {
                        content: get_chinese_piece_char(piece).to_string(),
                        position: Point::new(x, y),
                        color,
                        size: Pixels(SQUARE_SIZE * 0.6),
                        font: CHINESE_FONT,
                        horizontal_alignment: iced::alignment::Horizontal::Center,
                        vertical_alignment: iced::alignment::Vertical::Center,
                        ..canvas::Text::default()
                    };
                    let circle = canvas::Path::circle(Point::new(x, y), SQUARE_SIZE * 0.4); // Define the circle path
                    // Draw shadow for the piece
                    let shadow_offset = 3.0;
                    let shadow_circle = canvas::Path::circle(Point::new(x + shadow_offset, y + shadow_offset), SQUARE_SIZE * 0.4);
                    frame.fill(&shadow_circle, iced::Color::from_rgba8(0, 0, 0, 0.4)); // More visible semi-transparent black for shadow

                    // Draw solid background for the piece
                    frame.fill(&circle, iced::Color::from_rgb8(240, 240, 240)); // Slightly off-white background for circle
                    frame.stroke(&circle, Stroke::default().with_width(2.0).with_color(iced::Color::from_rgb8(0, 0, 0))); // Thicker black border
                    frame.fill_text(text);
                }
            }
        }
        
        // Highlight selected square
        if let Some(sq) = self.selected_square {
            let r = sq / 9;
            let c = sq % 9;
            let x = c as f32 * SQUARE_SIZE;
            let y = r as f32 * SQUARE_SIZE;
            let path = canvas::Path::rectangle(Point::new(x,y), Size::new(SQUARE_SIZE, SQUARE_SIZE));
            frame.stroke(&path, Stroke::default().with_width(3.0).with_color(iced::Color::from_rgb(0.0, 1.0, 0.0)));
        }


        vec![frame.into_geometry()]
    }
    
    fn update(
        &self,
        _state: &mut Self::State,
        event: CanvasEvent,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> (Status, Option<Message>) {
        if let CanvasEvent::Mouse(MouseEvent::ButtonPressed(iced::mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                let c = (pos.x / SQUARE_SIZE).floor() as usize;
                let r = (pos.y / SQUARE_SIZE).floor() as usize;
                if r < 10 && c < 9 {
                    let sq = r * 9 + c;
                    return (Status::Captured, Some(Message::SquareClicked(sq)));
                }
            }
        }
        (Status::Ignored, None)
    }
}