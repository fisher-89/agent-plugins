/**
 * Vitest global setup: keep plan generation offline and deterministic.
 * Tests that need real version probing must mockRestore / re-implement.
 */
import { afterAll, beforeAll, beforeEach, type Mock, vi } from 'vite-plus/test';

import * as testFramework from '../src/lib/test-framework';

let spy: Mock;

beforeAll(() => {
  spy = vi.spyOn(testFramework, 'detectFrameworkVersion');
});

beforeEach(() => {
  if (!vi.isMockFunction(testFramework.detectFrameworkVersion)) {
    spy = vi.spyOn(testFramework, 'detectFrameworkVersion');
  }
  spy.mockReset().mockReturnValue('99.0.0');
});

afterAll(() => {
  spy.mockRestore();
});
