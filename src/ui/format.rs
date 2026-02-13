use cozy_chess::Square;

pub fn format_square_display(sq: Square) -> String {
    let file = match sq.file() {
        cozy_chess::File::A => 'a',
        cozy_chess::File::B => 'b',
        cozy_chess::File::C => 'c',
        cozy_chess::File::D => 'd',
        cozy_chess::File::E => 'e',
        cozy_chess::File::F => 'f',
        cozy_chess::File::G => 'g',
        cozy_chess::File::H => 'h',
    };
    let rank = match sq.rank() {
        cozy_chess::Rank::First => '1',
        cozy_chess::Rank::Second => '2',
        cozy_chess::Rank::Third => '3',
        cozy_chess::Rank::Fourth => '4',
        cozy_chess::Rank::Fifth => '5',
        cozy_chess::Rank::Sixth => '6',
        cozy_chess::Rank::Seventh => '7',
        cozy_chess::Rank::Eighth => '8',
    };
    format!("{}{}", file, rank)
}
