use cozy_chess::{BitBoard, Board, Color, Piece, Rank, Square};
use smallvec::SmallVec;

use super::helpers::piece_attacks;

const MAX_ATTACKERS_PER_SQUARE: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attacker {
    pub from: Square,
    pub piece: Piece,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinInfo {
    pub pinner: Attacker,
    pub pinned: Square,
    pub pinned_to: Square,
    pub ray: BitBoard,
}

#[derive(Debug, Clone)]
pub struct AttackMap {
    attacked_by_white: [SmallVec<[Attacker; MAX_ATTACKERS_PER_SQUARE]>; 64],
    attacked_by_black: [SmallVec<[Attacker; MAX_ATTACKERS_PER_SQUARE]>; 64],
    pins: Vec<PinInfo>,
}

impl AttackMap {
    pub fn compute(board: &Board) -> Self {
        let mut attack_map = Self {
            attacked_by_white: std::array::from_fn(|_| SmallVec::new()),
            attacked_by_black: std::array::from_fn(|_| SmallVec::new()),
            pins: Vec::new(),
        };

        attack_map.populate_attacks(board);
        attack_map.pins = compute_pins(board);

        attack_map
    }

    pub fn attackers_of(&self, sq: Square, color: Color) -> &[Attacker] {
        let idx = square_index(sq);
        match color {
            Color::White => self.attacked_by_white[idx].as_slice(),
            Color::Black => self.attacked_by_black[idx].as_slice(),
        }
    }

    pub fn is_attacked(&self, sq: Square, by: Color) -> bool {
        !self.attackers_of(sq, by).is_empty()
    }

    pub fn pins(&self) -> &[PinInfo] {
        &self.pins
    }

    fn populate_attacks(&mut self, board: &Board) {
        for color in [Color::White, Color::Black] {
            for piece in Piece::ALL {
                let pieces = board.pieces(piece) & board.colors(color);
                for from in pieces {
                    let attacks = piece_attacks(board, from, piece, color);
                    for target in attacks {
                        let attacker = Attacker { from, piece };
                        let idx = square_index(target);
                        match color {
                            Color::White => self.attacked_by_white[idx].push(attacker),
                            Color::Black => self.attacked_by_black[idx].push(attacker),
                        }
                    }
                }
            }
        }
    }
}

fn square_index(sq: Square) -> usize {
    (sq.rank() as usize * 8) + sq.file() as usize
}

fn compute_pins(board: &Board) -> Vec<PinInfo> {
    let mut pins = Vec::new();

    for color in [Color::White, Color::Black] {
        let enemy = !color;

        for slider_piece in [Piece::Bishop, Piece::Rook, Piece::Queen] {
            let sliders = board.pieces(slider_piece) & board.colors(color);

            for slider_sq in sliders {
                let attacks = piece_attacks(board, slider_sq, slider_piece, color);
                let enemy_pieces = board.colors(enemy);

                for front_sq in attacks & enemy_pieces {
                    let Some(front_piece) = board.piece_on(front_sq) else {
                        continue;
                    };

                    let Some(back_sq) = find_piece_behind(board, slider_sq, front_sq, enemy) else {
                        continue;
                    };

                    let Some(back_piece) = board.piece_on(back_sq) else {
                        continue;
                    };

                    let is_pin = back_piece == Piece::King
                        || piece_value(back_piece) > piece_value(front_piece);
                    if is_pin {
                        pins.push(PinInfo {
                            pinner: Attacker {
                                from: slider_sq,
                                piece: slider_piece,
                            },
                            pinned: front_sq,
                            pinned_to: back_sq,
                            ray: ray_between_inclusive(slider_sq, back_sq),
                        });
                    }
                }
            }
        }
    }

    pins
}

fn find_piece_behind(
    board: &Board,
    slider_sq: Square,
    front_sq: Square,
    target_color: Color,
) -> Option<Square> {
    let slider_rank = slider_sq.rank() as i8;
    let slider_file = slider_sq.file() as i8;
    let front_rank = front_sq.rank() as i8;
    let front_file = front_sq.file() as i8;

    let dr = (front_rank - slider_rank).signum();
    let df = (front_file - slider_file).signum();

    if dr == 0 && df == 0 {
        return None;
    }

    let mut r = front_rank + dr;
    let mut f = front_file + df;

    while (0..8).contains(&r) && (0..8).contains(&f) {
        let rank = Rank::try_index(r as usize)?;
        let file = cozy_chess::File::try_index(f as usize)?;
        let sq = Square::new(file, rank);

        if board.occupied().has(sq) {
            if board.colors(target_color).has(sq) {
                return Some(sq);
            }
            return None;
        }

        r += dr;
        f += df;
    }

    None
}

fn ray_between_inclusive(from: Square, to: Square) -> BitBoard {
    let from_rank = from.rank() as i8;
    let from_file = from.file() as i8;
    let to_rank = to.rank() as i8;
    let to_file = to.file() as i8;

    let dr = (to_rank - from_rank).signum();
    let df = (to_file - from_file).signum();

    let mut rank = from_rank;
    let mut file = from_file;
    let mut ray = BitBoard::EMPTY;

    loop {
        let Some(rank_idx) = Rank::try_index(rank as usize) else {
            break;
        };
        let Some(file_idx) = cozy_chess::File::try_index(file as usize) else {
            break;
        };
        ray |= BitBoard::from(Square::new(file_idx, rank_idx));

        if rank == to_rank && file == to_file {
            break;
        }

        rank += dr;
        file += df;
    }

    ray
}

fn piece_value(piece: Piece) -> u16 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 330,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 20000,
    }
}

#[cfg(test)]
mod tests {
    use cozy_chess::{Board, Color, Square};

    use super::AttackMap;

    #[test]
    fn attackers_of_square_works() {
        let board = Board::default();
        let map = AttackMap::compute(&board);

        let white_attackers = map.attackers_of(Square::E3, Color::White);
        assert!(white_attackers.len() >= 2);

        let black_attackers = map.attackers_of(Square::E3, Color::Black);
        assert!(black_attackers.is_empty());
    }

    #[test]
    fn is_attacked_matches_attackers() {
        let board: Board = "4k3/8/8/3n4/8/5B2/8/4K3 w - - 0 1"
            .parse()
            .expect("valid fen");
        let map = AttackMap::compute(&board);

        assert!(map.is_attacked(Square::D5, Color::White));
        assert!(!map.is_attacked(Square::D5, Color::Black));
    }

    #[test]
    fn pin_detection_finds_classic_pin() {
        let board: Board = "4k3/8/2n5/8/B7/8/8/4K3 w - - 0 1"
            .parse()
            .expect("valid fen");
        let map = AttackMap::compute(&board);

        let pin = map
            .pins()
            .iter()
            .find(|pin| pin.pinned == Square::C6 && pin.pinned_to == Square::E8);

        assert!(pin.is_some(), "expected bishop pin from a4 to e8 via c6");
    }
}
