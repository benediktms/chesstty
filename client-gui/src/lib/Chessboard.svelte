<script lang="ts">
  import { onMount } from 'svelte';
  import { Chessground } from '@lichess-org/chessground';
  import type { Api } from '@lichess-org/chessground/api';
  import type { Config } from '@lichess-org/chessground/config';
  import type { Color, Dests, Key } from '@lichess-org/chessground/types';
  import type { LegalMove, Side } from './types';

  export let fen: string;
  export let orientation: Side = 'white';
  export let turnColor: Side = 'white';
  export let movableColor: Side | 'both' = 'both';
  export let legalMoves: LegalMove[] = [];
  export let lastMove: { from: string; to: string } | undefined;
  export let enabled = true;
  export let syncToken = 0;
  export let onmove: (from: string, to: string) => void;

  let element: HTMLButtonElement;
  let ground: Api | undefined;
  let keyboardInput = '';
  let keyboardMessage = '';

  function destinations(): Dests {
    const result: Dests = new Map();
    for (const move of legalMoves) {
      const from = move.from as Key;
      const to = move.to as Key;
      const destinations = result.get(from) ?? [];
      if (!destinations.includes(to)) destinations.push(to);
      result.set(from, destinations);
    }
    return result;
  }

  function config(): Config {
    return {
      fen: fen.split(' ')[0],
      orientation: orientation as Color,
      turnColor: turnColor as Color,
      lastMove: lastMove ? ([lastMove.from, lastMove.to] as Key[]) : undefined,
      animation: { enabled: true, duration: 180 },
      highlight: { lastMove: true, check: true },
      movable: {
        free: false,
        color: enabled ? (movableColor as Color | 'both') : undefined,
        dests: enabled ? destinations() : new Map(),
        showDests: true,
        events: { after: (from, to) => onmove(from, to) }
      },
      premovable: { enabled: false },
      draggable: { enabled: enabled, showGhost: true },
      selectable: { enabled: enabled }
    };
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!enabled) return;
    if (event.key === 'Escape') {
      keyboardInput = '';
      keyboardMessage = 'Move entry cleared';
      return;
    }
    if (event.key === 'Backspace') {
      event.preventDefault();
      keyboardInput = keyboardInput.slice(0, -1);
      keyboardMessage = keyboardInput || 'Type a move, for example e2e4';
      return;
    }

    const key = event.key.toLowerCase();
    const expectsFile = keyboardInput.length % 2 === 0;
    if (!(expectsFile ? /^[a-h]$/ : /^[1-8]$/).test(key)) return;
    event.preventDefault();
    keyboardInput += key;
    keyboardMessage = keyboardInput;

    if (keyboardInput.length === 4) {
      const from = keyboardInput.slice(0, 2);
      const to = keyboardInput.slice(2);
      keyboardInput = '';
      if (legalMoves.some((move) => move.from === from && move.to === to)) {
        keyboardMessage = `Playing ${from} to ${to}`;
        onmove(from, to);
      } else {
        keyboardMessage = `${from} to ${to} is not legal`;
      }
    }
  }

  onMount(() => {
    ground = Chessground(element, config());
    return () => ground?.destroy();
  });

  $: if (ground) {
    fen;
    orientation;
    turnColor;
    movableColor;
    legalMoves;
    lastMove;
    enabled;
    syncToken;
    ground.set(config());
  }
</script>

<div class="board-frame">
  <button
    type="button"
    bind:this={element}
    class="cg-wrap"
    aria-label={`Chess board, ${turnColor} to move`}
    onkeydown={handleKeydown}
    onfocus={() => (keyboardMessage = 'Type a move, for example e2e4')}
    onblur={() => {
      keyboardInput = '';
      keyboardMessage = '';
    }}
  ></button>
  {#if keyboardMessage}<span class="keyboard-message" aria-live="polite">{keyboardMessage}</span>{/if}
</div>

<style>
  .board-frame {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: 18px;
    box-shadow:
      0 24px 70px rgb(0 0 0 / 0.3),
      0 0 0 1px rgb(255 255 255 / 0.08);
  }

  .cg-wrap {
    display: block;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    color: inherit;
    background: transparent;
  }

  .keyboard-message {
    position: absolute;
    z-index: 12;
    top: 12px;
    left: 50%;
    padding: 7px 11px;
    border: 1px solid rgb(255 255 255 / 0.14);
    border-radius: 8px;
    color: #f5f0e8;
    background: rgb(19 23 19 / 0.88);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.3);
    font: 600 12px/1.2 ui-sans-serif, system-ui, sans-serif;
    transform: translateX(-50%);
    pointer-events: none;
  }
</style>
