/**
 * Tests for change-config.ts — workflow.json parsing.
 */

import * as fs from 'fs';

import { describe, it, expect, vi, beforeEach } from 'vite-plus/test';

vi.mock('fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, existsSync: vi.fn(), readFileSync: vi.fn() };
});

vi.mock('./change', () => ({
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

import { getWorkflowType } from './change-config';

function mockWorkflowJson(content: string | null): void {
  vi.mocked(fs.existsSync).mockImplementation((filePath: fs.PathLike) => {
    return String(filePath).endsWith('workflow.json') && content !== null;
  });
  vi.mocked(fs.readFileSync).mockImplementation(
    (
      path: fs.PathOrFileDescriptor,
      _options?: BufferEncoding | fs.ObjectEncodingOptions | null,
    ): string => {
      if (String(path).endsWith('workflow.json')) {
        return content ?? '';
      }
      return '';
    },
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('readWorkflowConfig — via getWorkflowType', () => {
  it('should parse workflow.json with workflow_type test-only', () => {
    mockWorkflowJson('{"workflow_type": "test-only"}');
    expect(getWorkflowType('my-change')).toBe('test-only');
  });
});

describe('getWorkflowType', () => {
  it('should return test-only when workflow.json contains test-only', () => {
    mockWorkflowJson('{"workflow_type": "test-only"}');
    expect(getWorkflowType('my-change')).toBe('test-only');
  });

  it('should return requirement when workflow.json contains requirement', () => {
    mockWorkflowJson('{"workflow_type": "requirement"}');
    expect(getWorkflowType('my-change')).toBe('requirement');
  });

  it('should return requirement when workflow.json file does not exist (AC-13)', () => {
    mockWorkflowJson(null);
    expect(getWorkflowType('my-change')).toBe('requirement');
  });

  it('should return requirement when workflow.json exists but workflow_type key is missing', () => {
    mockWorkflowJson('{}');
    expect(getWorkflowType('my-change')).toBe('requirement');
  });

  it('should return requirement when workflow_type is empty string', () => {
    mockWorkflowJson('{"workflow_type": ""}');
    expect(getWorkflowType('my-change')).toBe('requirement');
  });

  it('should throw when workflow.json contains invalid JSON', () => {
    mockWorkflowJson('{invalid json');
    expect(() => getWorkflowType('my-change')).toThrow(/workflow\.json 解析失败/);
  });
});
