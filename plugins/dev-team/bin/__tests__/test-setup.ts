/**
 * Vitest global setup: keep plan generation offline and deterministic.
 * Tests that need real version probing must mockRestore / re-implement.
 */
import { vi } from 'vite-plus/test';

import * as testFramework from '../src/lib/test-framework';

vi.spyOn(testFramework, 'detectFrameworkVersion').mockReturnValue('99.0.0');
