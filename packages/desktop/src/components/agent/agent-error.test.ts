import { describe, expect, it } from 'vite-plus/test';

import type { AgentEvent, TurnSummary } from '../../types/dto';
import { latestAgentFailure } from './agent-error';

const TS = 1_700_000_000_000;

function notice(seq: number, subtype: string, error: string, timestampMs = TS + seq): AgentEvent {
  return { seq, timestampMs, kind: 'systemNotice', subtype, payload: { error } };
}

function turnDone(
  seq: number,
  isError: boolean,
  subtype: string,
  timestampMs = TS + seq,
): AgentEvent {
  return {
    seq,
    timestampMs,
    kind: 'turnDone',
    subtype,
    isError,
    numTurns: 1,
    durationMs: 10,
    costUsd: null,
    usage: null,
    sessionId: 'sdk-1',
  };
}

function turn(
  status: TurnSummary['status'],
  options: { startedAt?: number; error?: string | null } = {},
): TurnSummary {
  const startedAt = options.startedAt ?? TS;
  return {
    turnId: 1,
    sessionId: 'ses-1',
    status,
    startedAt,
    finishedAt: status === 'running' ? null : startedAt + 10,
    numTurns: status === 'running' ? null : 1,
    costUsd: null,
    durationMs: status === 'running' ? null : 10,
    error: options.error ?? null,
  };
}

describe('latestAgentFailure：已知错误映射', () => {
  const cases: [string, string, string][] = [
    ['API 请求失败: RequestError: invalid uri character', 'Provider 地址无效', 'base_url'],
    [
      'API 流失败: HttpError: Invalid content type was returned: "text/html"',
      '端点返回了网页而不是 API 响应',
      'base_url',
    ],
    ['HttpError: 401 Unauthorized', 'Provider 认证失败', 'api_key'],
    ['HttpError: 429 rate limit exceeded', '请求过于频繁', '限流'],
    ['invalid_request_error: max_tokens too large', '输出上限被 Provider 拒绝', 'max_tokens'],
    ['something unexpected', 'Agent 运行失败', '原始错误'],
  ];

  for (const [raw, title, hint] of cases) {
    it(`${raw} → ${title}`, () => {
      const failure = latestAgentFailure([notice(1, 'api_error', raw)], [turn('failed')]);
      expect(failure?.title).toBe(title);
      expect(failure?.hint).toContain(hint);
      expect(failure?.raw).toBe(raw);
    });
  }
});

describe('latestAgentFailure：失败轮判定', () => {
  it('api_retry 不充当成因，取其后 api_error', () => {
    const failure = latestAgentFailure(
      [
        notice(1, 'api_retry', '已恢复的抖动'),
        notice(2, 'api_error', 'API 请求失败: RequestError: invalid uri character'),
      ],
      [turn('failed')],
    );
    expect(failure?.raw).toContain('invalid uri character');
    expect(failure?.raw).not.toContain('已恢复的抖动');
  });

  it('上一轮失败、最新轮已完成 → 不展示旧错误', () => {
    const failure = latestAgentFailure(
      [notice(1, 'api_error', '旧错误')],
      [turn('failed'), turn('completed', { startedAt: TS + 100 })],
    );
    expect(failure).toBeNull();
  });

  it('最新轮停止 → 不展示失败横幅', () => {
    expect(latestAgentFailure([], [turn('stopped')])).toBeNull();
  });

  it('无 api_error 时回落 TurnSummary.error', () => {
    const failure = latestAgentFailure([], [turn('failed', { error: '落库失败: db closed' })]);
    expect(failure?.raw).toBe('落库失败: db closed');
    expect(failure?.title).toBe('Agent 运行失败');
  });

  it('无 api_error 与 turn.error 时回落失败 TurnDone.subtype', () => {
    const failure = latestAgentFailure(
      [turnDone(1, true, 'error_during_execution')],
      [turn('failed')],
    );
    expect(failure?.raw).toBe('error_during_execution');
  });

  it('无轮行时以最后一条失败 TurnDone 兜底', () => {
    const failure = latestAgentFailure([turnDone(1, true, 'api_error')], []);
    expect(failure?.raw).toBe('api_error');
  });

  it('最新轮 running 但其失败 TurnDone 已到达 → 展示', () => {
    const failure = latestAgentFailure(
      [turnDone(1, true, 'api_error', TS + 5)],
      [turn('running', { startedAt: TS })],
    );
    expect(failure).not.toBeNull();
  });

  it('最新轮 running 但失败 TurnDone 属于上一轮 → 不展示', () => {
    const failure = latestAgentFailure(
      [turnDone(1, true, 'api_error', TS + 5)],
      [turn('running', { startedAt: TS + 100 })],
    );
    expect(failure).toBeNull();
  });

  it('失败但全无成因 → 仍返回通用横幅（raw 为空）', () => {
    const failure = latestAgentFailure([], [turn('failed')]);
    expect(failure?.title).toBe('Agent 运行失败');
    expect(failure?.raw).toBe('');
  });
});
