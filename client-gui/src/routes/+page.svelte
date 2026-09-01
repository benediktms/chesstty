<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import '@lichess-org/chessground/assets/chessground.base.css';
  import '@lichess-org/chessground/assets/chessground.brown.css';
  import '@lichess-org/chessground/assets/chessground.cburnett.css';
  import Chessboard from '$lib/Chessboard.svelte';
  import type { GameMode, GameState, MoveRecord, NewGameOptions, Side } from '$lib/types';
  import '../app.css';

  const pieces: Record<string, Record<Side, string>> = {
    p: { white: '♙', black: '♟' },
    n: { white: '♘', black: '♞' },
    b: { white: '♗', black: '♝' },
    r: { white: '♖', black: '♜' },
    q: { white: '♕', black: '♛' },
    k: { white: '♔', black: '♚' }
  };

  let game: GameState | undefined;
  let showMenu = true;
  let mode: GameMode = 'human-vs-engine';
  let humanSide: Side = 'white';
  let skillLevel = 10;
  let busy = false;
  let error = '';
  let syncToken = 0;
  let pendingPromotion: { from: string; to: string } | undefined;

  onMount(() => {
    let unlistenState: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    listen<GameState>('game-state', ({ payload }) => {
      game = payload;
      busy = false;
    }).then((unlisten) => (unlistenState = unlisten));
    listen<string>('game-error', ({ payload }) => {
      error = payload;
      busy = false;
      syncToken += 1;
    }).then((unlisten) => (unlistenError = unlisten));

    return () => {
      unlistenState?.();
      unlistenError?.();
    };
  });

  $: snapshot = game?.snapshot;
  $: isBotGame = snapshot?.gameMode === 1;
  $: orientation = (snapshot?.humanSide ?? 'white') as Side;
  $: playerCanMove = Boolean(
    snapshot &&
      snapshot.status === 0 &&
      !busy &&
      (!isBotGame || snapshot.sideToMove === snapshot.humanSide)
  );
  $: capturedByWhite = capturedPieces(snapshot?.history ?? [], 'white');
  $: capturedByBlack = capturedPieces(snapshot?.history ?? [], 'black');

  function skillName(level: number): string {
    if (level <= 3) return 'Beginner';
    if (level <= 10) return 'Intermediate';
    if (level <= 15) return 'Advanced';
    return 'Master';
  }

  function capturedPieces(history: MoveRecord[], mover: Side): string[] {
    return history.flatMap((move, index) => {
      const moveSide: Side = index % 2 === 0 ? 'white' : 'black';
      if (moveSide !== mover || !move.captured) return [];
      const capturedSide: Side = mover === 'white' ? 'black' : 'white';
      return pieces[move.captured.toLowerCase()]?.[capturedSide] ?? [];
    });
  }

  function gameStatus(): string {
    if (!snapshot) return '';
    if (snapshot.status === 2) return 'Draw';
    if (snapshot.status === 1) return `${snapshot.sideToMove === 'white' ? 'Black' : 'White'} wins`;
    if (snapshot.engineThinking) return 'Bot is thinking';
    return `${snapshot.sideToMove === 'white' ? 'White' : 'Black'} to move`;
  }

  async function startGame() {
    busy = true;
    error = '';
    const options: NewGameOptions = { mode, humanSide, skillLevel };
    try {
      game = await invoke<GameState>('new_game', { options });
      showMenu = false;
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  function requestMove(from: string, to: string) {
    const promotion = game?.legalMoves.some(
      (move) => move.from === from && move.to === to && move.promotion
    );
    if (promotion) {
      pendingPromotion = { from, to };
      return;
    }
    submitMove(from, to);
  }

  async function submitMove(from: string, to: string, promotion?: string) {
    busy = true;
    error = '';
    pendingPromotion = undefined;
    try {
      game = await invoke<GameState>('make_move', { from, to, promotion });
    } catch (cause) {
      error = String(cause);
      syncToken += 1;
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head>
  <title>ChessTTY</title>
  <meta
    name="description"
    content="A focused desktop chess client powered by the ChessTTY server"
  />
</svelte:head>

<main class:menu-open={showMenu}>
  <header class="app-bar">
    <a class="brand" href="/" aria-label="ChessTTY home">
      <span class="brand-mark" aria-hidden="true">♞</span>
      <span>ChessTTY</span>
    </a>
    {#if game}
      <button class="quiet-button" type="button" onclick={() => (showMenu = true)}>
        New game
      </button>
    {/if}
  </header>

  {#if snapshot}
    <section class="game-layout" aria-label="Current game">
      <div class="board-column">
        <div class="player-strip opponent">
          <span class="avatar">{orientation === 'white' ? '♟' : '♙'}</span>
          <div>
            <strong>{isBotGame ? `Stockfish · ${skillName(snapshot.skillLevel ?? skillLevel)}` : 'Black'}</strong>
            <small>{orientation === 'white' ? capturedByBlack.join(' ') || 'No captures' : capturedByWhite.join(' ') || 'No captures'}</small>
          </div>
          {#if snapshot.engineThinking}<span class="thinking-dot" aria-label="Thinking"></span>{/if}
        </div>

        <Chessboard
          fen={snapshot.fen}
          {orientation}
          turnColor={snapshot.sideToMove}
          movableColor={isBotGame ? (snapshot.humanSide ?? 'white') : 'both'}
          legalMoves={game?.legalMoves ?? []}
          lastMove={snapshot.lastMove}
          enabled={playerCanMove}
          {syncToken}
          onmove={requestMove}
        />

        <div class="player-strip">
          <span class="avatar light">{orientation === 'white' ? '♙' : '♟'}</span>
          <div>
            <strong>{isBotGame ? 'You' : 'White'}</strong>
            <small>{orientation === 'white' ? capturedByWhite.join(' ') || 'No captures' : capturedByBlack.join(' ') || 'No captures'}</small>
          </div>
        </div>
      </div>

      <aside class="game-sidebar">
        <section class="status-card">
          <p class="eyebrow">Game status</p>
          <div class="turn-status">
            <span class:dark-piece={snapshot.sideToMove === 'black'} class="turn-piece">
              {snapshot.sideToMove === 'white' ? '♙' : '♟'}
            </span>
            <div>
              <h1>{gameStatus()}</h1>
              <p>{isBotGame ? `Playing as ${snapshot.humanSide}` : 'Local two-player game'}</p>
            </div>
          </div>
        </section>

        <section class="history-card">
          <div class="section-heading">
            <div>
              <p class="eyebrow">Moves</p>
              <h2>Game history</h2>
            </div>
            <span>{snapshot.moveCount} plies</span>
          </div>
          <div class="move-list" aria-live="polite">
            {#if snapshot.history.length === 0}
              <div class="empty-history">
                <span aria-hidden="true">♙</span>
                <p>Your moves will appear here.</p>
              </div>
            {:else}
              {#each snapshot.history as move, index}
                <div class="move-row" class:last={index === snapshot.history.length - 1}>
                  <span>{Math.floor(index / 2) + 1}{index % 2 === 0 ? '.' : '…'}</span>
                  <strong>{move.san || `${move.from}–${move.to}`}</strong>
                  {#if move.captured}<small>capture</small>{/if}
                </div>
              {/each}
            {/if}
          </div>
        </section>

        {#if error}<p class="inline-error" role="alert">{error}</p>{/if}
      </aside>
    </section>
  {:else}
    <section class="welcome" aria-hidden={showMenu}>
      <span class="hero-piece">♞</span>
      <h1>Ready when you are.</h1>
    </section>
  {/if}

  {#if showMenu}
    <div class="menu-backdrop" role="presentation">
      <div class="new-game-sheet" role="dialog" aria-modal="true" aria-labelledby="new-game-title" tabindex="-1">
        <div class="sheet-intro">
          <p class="eyebrow">ChessTTY</p>
          <h1 id="new-game-title">Start a new game</h1>
          <p>Choose how you want to play. You can change everything again next game.</p>
        </div>

        <form onsubmit={(event) => { event.preventDefault(); startGame(); }}>
          <fieldset>
            <legend>Opponent</legend>
            <div class="choice-grid">
              <label class:chosen={mode === 'human-vs-engine'}>
                <input type="radio" bind:group={mode} value="human-vs-engine" />
                <span class="choice-icon">♜</span>
                <strong>Play the bot</strong>
                <small>A focused game against Stockfish</small>
              </label>
              <label class:chosen={mode === 'human-vs-human'}>
                <input type="radio" bind:group={mode} value="human-vs-human" />
                <span class="choice-icon">♙</span>
                <strong>Two players</strong>
                <small>Share this board locally</small>
              </label>
            </div>
          </fieldset>

          {#if mode === 'human-vs-engine'}
            <fieldset>
              <legend>Play as</legend>
              <div class="segmented-control">
                <label class:chosen={humanSide === 'white'}>
                  <input type="radio" bind:group={humanSide} value="white" />
                  <span>♙ White</span>
                </label>
                <label class:chosen={humanSide === 'black'}>
                  <input type="radio" bind:group={humanSide} value="black" />
                  <span>♟ Black</span>
                </label>
              </div>
            </fieldset>

            <fieldset class="strength-field">
              <div class="legend-row">
                <legend>Bot strength</legend>
                <output for="skill-level">{skillName(skillLevel)} · {skillLevel}</output>
              </div>
              <input id="skill-level" type="range" min="0" max="20" step="1" bind:value={skillLevel} />
              <div class="range-labels"><span>Gentle</span><span>Unforgiving</span></div>
            </fieldset>
          {/if}

          {#if error}<p class="form-error" role="alert">{error}</p>{/if}

          <div class="sheet-actions">
            {#if game}
              <button class="secondary-button" type="button" onclick={() => (showMenu = false)}>Cancel</button>
            {/if}
            <button class="primary-button" type="submit" disabled={busy}>
              {busy ? 'Starting…' : 'Start game'}
              <span aria-hidden="true">→</span>
            </button>
          </div>
        </form>
      </div>
    </div>
  {/if}

  {#if pendingPromotion}
    <div class="menu-backdrop promotion-backdrop">
      <div class="promotion-dialog" role="dialog" aria-modal="true" aria-labelledby="promotion-title" tabindex="-1">
        <p class="eyebrow">Promotion</p>
        <h2 id="promotion-title">Choose a piece</h2>
        <div class="promotion-options">
          {#each ['q', 'r', 'b', 'n'] as piece}
            <button
              type="button"
              aria-label={`Promote to ${piece}`}
              onclick={() => submitMove(pendingPromotion!.from, pendingPromotion!.to, piece)}
            >
              {pieces[piece][snapshot?.sideToMove ?? 'white']}
            </button>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</main>
