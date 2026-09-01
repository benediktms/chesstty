export type Side = 'white' | 'black';
export type GameMode = 'human-vs-engine' | 'human-vs-human';

export interface NewGameOptions {
  mode: GameMode;
  humanSide: Side;
  skillLevel: number;
}

export interface MoveRecord {
  from: string;
  to: string;
  piece: string;
  captured?: string;
  san: string;
  promotion?: string;
}

export interface LegalMove {
  from: string;
  to: string;
  promotion?: string;
  isCapture: boolean;
}

export interface GameSnapshot {
  sessionId: string;
  fen: string;
  sideToMove: Side;
  phase: number;
  status: number;
  moveCount: number;
  history: MoveRecord[];
  lastMove?: { from: string; to: string };
  gameMode: number;
  humanSide?: Side;
  engineThinking: boolean;
  skillLevel?: number;
  startFen: string;
}

export interface GameState {
  snapshot: GameSnapshot;
  legalMoves: LegalMove[];
}
