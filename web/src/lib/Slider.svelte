<script lang="ts">
  // A labelled slider with a live readout. With `stops`, it snaps to those values (for
  // ranges like 50 to 20,000 where every number in between isn't useful); a current value
  // that isn't a stop is kept as one, so opening a form never changes a setting by itself.
  import { formatCount } from './format'

  let {
    label,
    value = $bindable(),
    min = 1,
    max = 100,
    step = 1,
    stops,
    unit = ''
  }: { label: string; value: number; min?: number; max?: number; step?: number; stops?: number[]; unit?: string } = $props()

  const id = `s-${Math.random().toString(36).slice(2, 8)}`
  let points = $derived(stops ? [...new Set([...stops, value])].sort((a, b) => a - b) : null)
  let pos = $derived(points ? points.indexOf(value) : value)
  let lo = $derived(points ? 0 : min)
  let hi = $derived(points ? points.length - 1 : max)
  let pct = $derived(hi > lo ? ((pos - lo) / (hi - lo)) * 100 : 0)

  function oninput(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value)
    value = points ? points[v]! : v
  }
</script>

<div class="slider">
  <div class="top">
    <label for={id}>{label}</label>
    <output for={id}>{formatCount(value)}{#if unit}<small>{unit}</small>{/if}</output>
  </div>
  <input
    {id}
    type="range"
    min={lo}
    max={hi}
    step={points ? 1 : step}
    value={pos}
    aria-valuetext="{value} {unit}"
    style:--pct="{pct}%"
    {oninput}
  />
  <div class="ends" aria-hidden="true">
    <span>{formatCount(points ? points[0]! : min)}</span><span>{formatCount(points ? points[points.length - 1]! : max)}</span>
  </div>
</div>

<style>
  .slider {
    display: grid;
    gap: 6px;
  }
  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }
  label {
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  output {
    font: 400 26px/1 var(--term);
    color: var(--white);
  }
  output small {
    font-size: 16px;
    color: var(--text-2);
    margin-left: 4px;
  }
  input {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 22px;
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  input::-webkit-slider-runnable-track {
    height: 6px;
    background: linear-gradient(to right, var(--text) var(--pct), var(--line) var(--pct));
  }
  input::-moz-range-track {
    height: 6px;
    background: linear-gradient(to right, var(--text) var(--pct), var(--line) var(--pct));
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 22px;
    margin-top: -8px;
    background: var(--white);
    border: 0;
    box-shadow: 3px 3px 0 var(--line);
  }
  input::-moz-range-thumb {
    width: 14px;
    height: 22px;
    background: var(--white);
    border: 0;
    box-shadow: 3px 3px 0 var(--line);
  }
  input:focus-visible::-webkit-slider-thumb,
  input:active::-webkit-slider-thumb {
    background: var(--red);
  }
  input:focus-visible::-moz-range-thumb,
  input:active::-moz-range-thumb {
    background: var(--red);
  }
  @media (hover: hover) and (pointer: fine) {
    input:hover::-webkit-slider-thumb {
      background: var(--red);
    }
    input:hover::-moz-range-thumb {
      background: var(--red);
    }
  }
  input:focus-visible {
    outline: 1px solid var(--white);
    outline-offset: 4px;
  }
  .ends {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    color: var(--text-3);
  }
</style>
