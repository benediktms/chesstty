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

  let element: HTMLDivElement;
  let ground: Api | undefined;

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
  <div
    bind:this={element}
    class="cg-wrap"
    role="img"
    aria-label={`Chess board, ${turnColor} to move`}
  ></div>
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

</style>
