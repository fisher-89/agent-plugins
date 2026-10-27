// 本文件由 `cargo run -p export-bindings` 生成（specta-zod 0.0.4 + PATCH(1..3)），勿手改。
// 纯 schema 产物：只出 schema，不出类型别名、不做类型推断（类型唯一源是 `./bindings.ts`）。
// 装饰器（default / transform / catch / pipe / refine）一律禁用：`z.intersection` 的合并
// 语义在两侧装饰冲突时会 throw，soft-report 链路 MUST NOT 因此在自身内部崩掉。
// IPC 类型唯一源是 `./bindings.ts`；本文件只供 `src/lib/ipc.ts` 做运行期校验。
import { z } from "zod";

/** 降级哨兵：运行时等价 `z.unknown()`，`meta.degraded` 记原因（可 grep / 可 `.meta()` 回读） */
const degraded = (reason: string) => z.unknown().meta({ degraded: reason });

/** 逐命令参数 schema（键序 = Rust 形参序；键名 = invoke payload 键名 lowerCamelCase） */
export const argsSchemas = {
	get "listChanges"() { return z.strictObject({
			"root": z.string(),
		}); },
	get "getChangeDetail"() { return z.strictObject({
			"root": z.string(),
			"id": z.string(),
		}); },
	get "readArtifact"() { return z.strictObject({
			"root": z.string(),
			"id": z.string(),
			"kind": z.string(),
			"source": z.string(),
		}); },
	get "readExecOutput"() { return z.strictObject({
			"root": z.string(),
			"changeName": z.string(),
			"execId": z.string(),
		}); },
	get "archiveChange"() { return z.strictObject({
			"root": z.string(),
			"id": z.string(),
		}); },
	get "listWorkspaces"() { return z.strictObject({
		}); },
	get "addWorkspace"() { return z.strictObject({
			"root": z.string(),
		}); },
	get "removeWorkspace"() { return z.strictObject({
			"root": z.string(),
		}); },
	get "listAgentProviders"() { return z.strictObject({
		}); },
	get "saveAgentProvider"() { return z.strictObject({
			"id": z.number().nullable(),
			"name": z.string(),
			"protocol": z.lazy(() => AgentProviderProtocolSchema),
			"baseUrl": z.string(),
			"apiKey": z.string(),
			"models": z.lazy(() => AgentModelTiersSchema),
			"contextLength": z.number().nullable(),
			"maxTokens": z.number().nullable(),
			"webSearch": z.boolean(),
		}); },
	get "deleteAgentProvider"() { return z.strictObject({
			"id": z.number(),
		}); },
	get "listAgentInstances"() { return z.strictObject({
		}); },
	get "saveAgentInstance"() { return z.strictObject({
			"id": z.number().nullable(),
			"name": z.string(),
			"engine": z.lazy(() => AgentEngineKindSchema),
			"providerId": z.number().nullable(),
		}); },
	get "deleteAgentInstance"() { return z.strictObject({
			"id": z.number(),
		}); },
	get "setDefaultAgentInstance"() { return z.strictObject({
			"id": z.number(),
		}); },
	get "agentStart"() { return z.strictObject({
			"onEvent": degraded("channel: 非 JSON 可表达，跳过值校验"),
			"root": z.string(),
			"prompt": z.string(),
			"permissionMode": z.lazy(() => AgentPermissionModeSchema),
			"sessionId": z.string().nullable(),
			"source": z.string().nullable(),
			"sourceRef": z.string().nullable(),
			"agent": z.number().nullable(),
			"cwd": z.string().nullable(),
		}); },
	get "agentStop"() { return z.strictObject({
			"root": z.string(),
			"sessionId": z.string(),
		}); },
	get "agentSessionWatch"() { return z.strictObject({
			"onEvent": degraded("channel: 非 JSON 可表达，跳过值校验"),
			"root": z.string(),
			"sessionId": z.string(),
			"sinceSeq": z.number(),
		}); },
	get "agentSessions"() { return z.strictObject({
			"root": z.string(),
			"source": z.string().nullable(),
			"sourceRef": z.string().nullable(),
			"offset": z.int().min(0).max(4294967295),
			"limit": z.int().min(0).max(4294967295),
		}); },
	get "agentSessionTranscript"() { return z.strictObject({
			"root": z.string(),
			"sessionId": z.string(),
		}); },
	get "sessionDetail"() { return z.strictObject({
			"root": z.string(),
			"sessionId": z.string(),
		}); },
	get "listChangeChatSessions"() { return z.strictObject({
			"root": z.string(),
			"id": z.string(),
			"offset": z.int().min(0).max(4294967295),
			"limit": z.int().min(0).max(4294967295),
		}); },
	get "changeFlowStart"() { return z.strictObject({
			"onEvent": degraded("channel: 非 JSON 可表达，跳过值校验"),
			"root": z.string(),
			"id": z.string(),
			"autoNextPhase": z.boolean(),
		}); },
	get "changeFlowStop"() { return z.strictObject({
			"root": z.string(),
			"id": z.string(),
		}); },
	get "changeFlowAnswer"() { return z.strictObject({
			"root": z.string(),
			"id": z.string(),
			"answer": z.string(),
		}); },
	get "changeFlowConfirm"() { return z.strictObject({
			"root": z.string(),
			"id": z.string(),
			"proceed": z.boolean(),
			"syncSpecs": z.boolean(),
		}); },
	get "changeFlowWatch"() { return z.strictObject({
			"onEvent": degraded("channel: 非 JSON 可表达，跳过值校验"),
			"root": z.string(),
			"id": z.string(),
		}); },
	get "createChange"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
			"goal": z.string(),
			"workflowType": z.string(),
		}); },
	get "readExplore"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
		}); },
	get "scanExplores"() { return z.strictObject({
			"root": z.string(),
		}); },
	get "exploreDocPath"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
		}); },
	get "listExploreRecords"() { return z.strictObject({
			"root": z.string(),
		}); },
	get "createExploreRecord"() { return z.strictObject({
			"root": z.string(),
			"name": z.string().nullable(),
		}); },
	get "renameExploreRecord"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
			"newName": z.string(),
		}); },
	get "deleteExploreRecord"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
		}); },
	get "updateExploreTitle"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
			"title": z.string(),
		}); },
	get "updateExploreWorkflowType"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
			"workflowType": z.string(),
		}); },
	get "ensureExploreSession"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
		}); },
	get "promoteExplore"() { return z.strictObject({
			"root": z.string(),
			"name": z.string(),
			"workflowType": z.string(),
		}); },
	get "watchSubscribe"() { return z.strictObject({
			"onEvent": degraded("channel: 非 JSON 可表达，跳过值校验"),
			"path": z.string(),
		}); },
	get "watchUnsubscribe"() { return z.strictObject({
			"subscriptionId": z.number(),
		}); },
	get "dbModels"() { return z.strictObject({
			"scope": z.lazy(() => DbDimensionSchema),
			"root": z.string(),
		}); },
	get "dbRecords"() { return z.strictObject({
			"scope": z.lazy(() => DbDimensionSchema),
			"root": z.string(),
			"model": z.string(),
			"offset": z.int().min(0).max(4294967295),
			"limit": z.int().min(0).max(4294967295),
		}); },
	get "codeStats"() { return z.strictObject({
			"root": z.string(),
			"depth": z.int().min(0).max(4294967295),
		}); },
	get "workspaceConfig"() { return z.strictObject({
			"root": z.string(),
		}); },
	get "appEnvironment"() { return z.strictObject({
		}); },
	get "archiGraph"() { return z.strictObject({
			"root": z.string(),
		}); },
	get "archiModule"() { return z.strictObject({
			"root": z.string(),
			"element": z.string(),
		}); },
	get "archiReadDoc"() { return z.strictObject({
			"root": z.string(),
			"path": z.string(),
		}); },
	get "archiChangeSummary"() { return z.strictObject({
			"root": z.string(),
			"changeId": z.string(),
		}); },
	get "archiSetGate"() { return z.strictObject({
			"root": z.string(),
			"element": z.string(),
			"gate": z.lazy(() => GateTierSchema),
		}); },
} as const;

/** 逐命令结果 schema（`Result` 取 Ok 臂；`Option` 出 nullable） */
export const resultSchemas = {
	get "listChanges"() { return z.lazy(() => ChangeListSchema); },
	get "getChangeDetail"() { return z.lazy(() => ChangeDetailUnifiedSchema).nullable(); },
	get "readArtifact"() { return z.lazy(() => ArtifactEnvelopeSchema).nullable(); },
	get "readExecOutput"() { return z.string().nullable(); },
	get "archiveChange"() { return z.lazy(() => ArchiveOutcomeSchema); },
	get "listWorkspaces"() { return z.array(z.lazy(() => WorkspaceRecordSchema)); },
	get "addWorkspace"() { return z.lazy(() => WorkspaceRecordSchema); },
	get "removeWorkspace"() { return z.boolean(); },
	get "listAgentProviders"() { return z.array(z.lazy(() => AgentProviderRecordSchema)); },
	get "saveAgentProvider"() { return z.lazy(() => AgentProviderRecordSchema); },
	get "deleteAgentProvider"() { return z.boolean(); },
	get "listAgentInstances"() { return z.array(z.lazy(() => AgentInstanceRecordSchema)); },
	get "saveAgentInstance"() { return z.lazy(() => AgentInstanceRecordSchema); },
	get "deleteAgentInstance"() { return z.boolean(); },
	get "setDefaultAgentInstance"() { return z.lazy(() => AgentInstanceRecordSchema); },
	get "agentStart"() { return z.lazy(() => TurnSummarySchema); },
	get "agentStop"() { return z.null(); },
	get "agentSessionWatch"() { return z.null(); },
	get "agentSessions"() { return z.lazy(() => SessionPageSchema); },
	get "agentSessionTranscript"() { return z.array(z.lazy(() => AgentEventSchema)); },
	get "sessionDetail"() { return z.lazy(() => SessionSummarySchema).nullable(); },
	get "listChangeChatSessions"() { return z.lazy(() => SessionPageSchema); },
	get "changeFlowStart"() { return z.lazy(() => ChangeRunSummarySchema); },
	get "changeFlowStop"() { return z.null(); },
	get "changeFlowAnswer"() { return z.null(); },
	get "changeFlowConfirm"() { return z.null(); },
	get "changeFlowWatch"() { return z.null(); },
	get "createChange"() { return z.lazy(() => CreateOutcomeSchema); },
	get "readExplore"() { return z.lazy(() => ExploreDocSchema).nullable(); },
	get "scanExplores"() { return z.array(z.lazy(() => ExploreScanEntrySchema)); },
	get "exploreDocPath"() { return z.string().nullable(); },
	get "listExploreRecords"() { return z.array(z.lazy(() => ExploreRecordSchema)); },
	get "createExploreRecord"() { return z.lazy(() => ExploreRecordSchema); },
	get "renameExploreRecord"() { return z.lazy(() => ExploreRecordSchema); },
	get "deleteExploreRecord"() { return z.boolean(); },
	get "updateExploreTitle"() { return z.lazy(() => ExploreRecordSchema); },
	get "updateExploreWorkflowType"() { return z.lazy(() => ExploreRecordSchema); },
	get "ensureExploreSession"() { return z.lazy(() => ExploreRecordSchema); },
	get "promoteExplore"() { return z.lazy(() => PromoteOutcomeSchema); },
	get "watchSubscribe"() { return z.number(); },
	get "watchUnsubscribe"() { return z.boolean(); },
	get "dbModels"() { return z.array(z.lazy(() => ModelInfoSchema)); },
	get "dbRecords"() { return z.lazy(() => RecordsPageSchema); },
	get "codeStats"() { return z.lazy(() => CodeStatsReportSchema); },
	get "workspaceConfig"() { return z.lazy(() => WorkspaceConfigReportSchema); },
	get "appEnvironment"() { return z.lazy(() => AppEnvironmentSchema); },
	get "archiGraph"() { return z.lazy(() => ArchiGraphSchema); },
	get "archiModule"() { return z.lazy(() => ArchiModuleDetailSchema); },
	get "archiReadDoc"() { return z.string(); },
	get "archiChangeSummary"() { return z.lazy(() => ArchiChangeSummarySchema).nullable(); },
	get "archiSetGate"() { return z.null(); },
} as const;

/** 逐命令 `Channel<T>` 形参的信封 schema（键 = 形参名；无 Channel 的命令为空对象） */
export const channelArgs = {
	get "listChanges"() { return {
		}; },
	get "getChangeDetail"() { return {
		}; },
	get "readArtifact"() { return {
		}; },
	get "readExecOutput"() { return {
		}; },
	get "archiveChange"() { return {
		}; },
	get "listWorkspaces"() { return {
		}; },
	get "addWorkspace"() { return {
		}; },
	get "removeWorkspace"() { return {
		}; },
	get "listAgentProviders"() { return {
		}; },
	get "saveAgentProvider"() { return {
		}; },
	get "deleteAgentProvider"() { return {
		}; },
	get "listAgentInstances"() { return {
		}; },
	get "saveAgentInstance"() { return {
		}; },
	get "deleteAgentInstance"() { return {
		}; },
	get "setDefaultAgentInstance"() { return {
		}; },
	get "agentStart"() { return {
			get "onEvent"() { return z.lazy(() => AgentRunMessageSchema); },
		}; },
	get "agentStop"() { return {
		}; },
	get "agentSessionWatch"() { return {
			get "onEvent"() { return z.lazy(() => AgentRunMessageSchema); },
		}; },
	get "agentSessions"() { return {
		}; },
	get "agentSessionTranscript"() { return {
		}; },
	get "sessionDetail"() { return {
		}; },
	get "listChangeChatSessions"() { return {
		}; },
	get "changeFlowStart"() { return {
			get "onEvent"() { return z.lazy(() => RunNoticeSchema); },
		}; },
	get "changeFlowStop"() { return {
		}; },
	get "changeFlowAnswer"() { return {
		}; },
	get "changeFlowConfirm"() { return {
		}; },
	get "changeFlowWatch"() { return {
			get "onEvent"() { return z.lazy(() => RunNoticeSchema); },
		}; },
	get "createChange"() { return {
		}; },
	get "readExplore"() { return {
		}; },
	get "scanExplores"() { return {
		}; },
	get "exploreDocPath"() { return {
		}; },
	get "listExploreRecords"() { return {
		}; },
	get "createExploreRecord"() { return {
		}; },
	get "renameExploreRecord"() { return {
		}; },
	get "deleteExploreRecord"() { return {
		}; },
	get "updateExploreTitle"() { return {
		}; },
	get "updateExploreWorkflowType"() { return {
		}; },
	get "ensureExploreSession"() { return {
		}; },
	get "promoteExplore"() { return {
		}; },
	get "watchSubscribe"() { return {
			get "onEvent"() { return z.lazy(() => FileWatchEventSchema); },
		}; },
	get "watchUnsubscribe"() { return {
		}; },
	get "dbModels"() { return {
		}; },
	get "dbRecords"() { return {
		}; },
	get "codeStats"() { return {
		}; },
	get "workspaceConfig"() { return {
		}; },
	get "appEnvironment"() { return {
		}; },
	get "archiGraph"() { return {
		}; },
	get "archiModule"() { return {
		}; },
	get "archiReadDoc"() { return {
		}; },
	get "archiChangeSummary"() { return {
		}; },
	get "archiSetGate"() { return {
		}; },
} as const;
export const ActivePhaseSchema = z.strictObject({
	get phase() { return z.string(); },
	get attempt() { return z.int().min(0).max(4294967295); },
	get startAt() { return z.string().nullable(); },
});

export const ActiveRunViewSchema = z.strictObject({
	get runId() { return z.string(); },
	get status() { return ChangeRunStatusSchema; },
	get phase() { return z.string().nullable(); },
	get attempt() { return z.int().min(0).max(4294967295).nullable(); },
	get ask() { return AskPayloadSchema.nullable(); },
	get archive() { return ArchivePreflightSchema.nullable(); },
	get startedAt() { return z.string(); },
	get nodes() { return z.array(ChangeStepStateSchema); },
	get edges() { return z.array(ChangeRunEdgeRecordSchema); },
});

export const AgentBlockSchema = z.union([z.strictObject({
	get kind() { return z.literal("text"); },
	get text() { return z.string(); },
	get citations() { return z.array(AgentCitationSchema).nullable().optional(); },
}), z.strictObject({
	get kind() { return z.literal("thinking"); },
	get thinking() { return z.string(); },
}), z.strictObject({
	get kind() { return z.literal("toolUse"); },
	get id() { return z.string(); },
	get name() { return z.string(); },
	get input() { return z.unknown(); },
}), z.strictObject({
	get kind() { return z.literal("toolResult"); },
	get id() { return z.string(); },
	get content() { return z.string(); },
	get isError() { return z.boolean(); },
}), z.strictObject({
	get kind() { return z.literal("webSearch"); },
	get id() { return z.string(); },
	get query() { return z.string(); },
	get results() { return z.array(WebSearchSourceSchema).optional(); },
	get error() { return z.string().nullable().optional(); },
	get rawUse() { return z.unknown().nullable().optional(); },
	get rawResult() { return z.unknown().nullable().optional(); },
})]);

export const AgentCitationSchema = z.strictObject({
	get url() { return z.string().nullable().optional(); },
	get title() { return z.string().nullable().optional(); },
	get citedText() { return z.string().nullable().optional(); },
	get raw() { return z.unknown(); },
});

export const AgentDeltaSchema = z.union([z.strictObject({
	get kind() { return z.literal("text"); },
	get text() { return z.string(); },
}), z.strictObject({
	get kind() { return z.literal("thinking"); },
	get thinking() { return z.string(); },
})]);

export const AgentEngineKindSchema = z.union([z.literal("cli"), z.literal("sdk")]);

export const AgentEventSchema = z.strictObject({
	get seq() { return z.number(); },
	get timestampMs() { return z.number(); },
}).and(z.union([z.strictObject({
	get kind() { return z.literal("runStarted"); },
	get model() { return z.string().nullable(); },
	get sessionId() { return z.string().nullable(); },
	get tools() { return z.array(z.string()); },
	get mcpServers() { return z.array(z.string()); },
	seq: z.unknown().optional(),
	timestampMs: z.unknown().optional(),
}), z.strictObject({
	get kind() { return z.literal("messageDelta"); },
	get parentToolUseId() { return z.string().nullable(); },
	get delta() { return AgentDeltaSchema; },
	seq: z.unknown().optional(),
	timestampMs: z.unknown().optional(),
}), z.strictObject({
	get kind() { return z.literal("message"); },
	get role() { return AgentMessageRoleSchema; },
	get blocks() { return z.array(AgentBlockSchema); },
	get parentToolUseId() { return z.string().nullable(); },
	seq: z.unknown().optional(),
	timestampMs: z.unknown().optional(),
}), z.strictObject({
	get kind() { return z.literal("systemNotice"); },
	get subtype() { return z.string(); },
	get payload() { return z.unknown(); },
	seq: z.unknown().optional(),
	timestampMs: z.unknown().optional(),
}), z.strictObject({
	get kind() { return z.literal("turnDone"); },
	get subtype() { return z.string(); },
	get isError() { return z.boolean(); },
	get numTurns() { return z.number().nullable(); },
	get durationMs() { return z.number().nullable(); },
	get costUsd() { return z.number().nullable().nullable(); },
	get usage() { return z.unknown(); },
	get sessionId() { return z.string().nullable(); },
	seq: z.unknown().optional(),
	timestampMs: z.unknown().optional(),
}), z.strictObject({
	get kind() { return z.literal("raw"); },
	get eventType() { return z.string(); },
	get rawJson() { return z.string(); },
	seq: z.unknown().optional(),
	timestampMs: z.unknown().optional(),
})]));

export const AgentEventKindSchema = z.union([z.strictObject({
	get kind() { return z.literal("runStarted"); },
	get model() { return z.string().nullable(); },
	get sessionId() { return z.string().nullable(); },
	get tools() { return z.array(z.string()); },
	get mcpServers() { return z.array(z.string()); },
}), z.strictObject({
	get kind() { return z.literal("messageDelta"); },
	get parentToolUseId() { return z.string().nullable(); },
	get delta() { return AgentDeltaSchema; },
}), z.strictObject({
	get kind() { return z.literal("message"); },
	get role() { return AgentMessageRoleSchema; },
	get blocks() { return z.array(AgentBlockSchema); },
	get parentToolUseId() { return z.string().nullable(); },
}), z.strictObject({
	get kind() { return z.literal("systemNotice"); },
	get subtype() { return z.string(); },
	get payload() { return z.unknown(); },
}), z.strictObject({
	get kind() { return z.literal("turnDone"); },
	get subtype() { return z.string(); },
	get isError() { return z.boolean(); },
	get numTurns() { return z.number().nullable(); },
	get durationMs() { return z.number().nullable(); },
	get costUsd() { return z.number().nullable().nullable(); },
	get usage() { return z.unknown(); },
	get sessionId() { return z.string().nullable(); },
}), z.strictObject({
	get kind() { return z.literal("raw"); },
	get eventType() { return z.string(); },
	get rawJson() { return z.string(); },
})]);

export const AgentInstanceRecordSchema = z.strictObject({
	get id() { return z.number(); },
	get name() { return z.string(); },
	get engine() { return AgentEngineKindSchema; },
	get providerId() { return z.number().nullable(); },
	get isDefault() { return z.boolean(); },
});

export const AgentMessageRoleSchema = z.union([z.literal("user"), z.literal("assistant"), z.literal("tool")]);

export const AgentModelTiersSchema = z.strictObject({
	get high() { return z.string(); },
	get medium() { return z.string(); },
	get low() { return z.string(); },
});

export const AgentPermissionModeSchema = z.union([z.literal("default"), z.literal("acceptEdits"), z.literal("bypassPermissions")]);

export const AgentProviderProtocolSchema = z.union([z.literal("openai"), z.literal("anthropic")]);

export const AgentProviderRecordSchema = z.strictObject({
	get id() { return z.number(); },
	get name() { return z.string(); },
	get protocol() { return AgentProviderProtocolSchema.optional(); },
	get baseUrl() { return z.string(); },
	get apiKey() { return z.string(); },
	get models() { return AgentModelTiersSchema; },
	get contextLength() { return z.number().nullable().optional(); },
	get maxTokens() { return z.number().nullable().optional(); },
	get webSearch() { return z.boolean().optional(); },
});

export const AgentRunMessageSchema = z.union([z.strictObject({
	get ipc() { return z.literal("event"); },
	get event() { return AgentEventSchema; },
}), z.strictObject({
	get ipc() { return z.literal("record"); },
	get record() { return TurnSummarySchema; },
})]);

export const AgentRunStatusSchema = z.union([z.literal("running"), z.literal("completed"), z.literal("failed"), z.literal("stopped")]);

export const AppEnvironmentSchema = z.strictObject({
	get mode() { return z.string(); },
	get dataRoot() { return z.string(); },
	get source() { return z.string(); },
});

export const ArchiChangeSummarySchema = z.strictObject({
	get elementsAdded() { return z.array(z.string()); },
	get elementsRemoved() { return z.array(z.string()); },
	get relationsAdded() { return z.int().min(0).max(4294967295); },
	get relationsRemoved() { return z.int().min(0).max(4294967295); },
	get gateChanges() { return z.array(ArchiGateChangeSchema); },
	get diagnostics() { return z.array(ArchiDiagnosticSchema); },
});

export const ArchiDependedBySchema = z.strictObject({
	get source() { return z.string(); },
	get description() { return z.string().nullable(); },
});

export const ArchiDependsOnSchema = z.strictObject({
	get target() { return z.string(); },
	get description() { return z.string().nullable(); },
});

export const ArchiDiagnosticSchema = z.strictObject({
	get kind() { return ArchiDiagnosticKindSchema; },
	get message() { return z.string(); },
	get file() { return z.string(); },
	get line() { return z.int().min(0).max(4294967295); },
	get column() { return z.int().min(0).max(4294967295).nullable(); },
});

export const ArchiDiagnosticKindSchema = z.union([z.literal("syntaxError"), z.literal("missingField"), z.literal("invalidValue"), z.literal("unknownField"), z.literal("duplicateElement"), z.literal("cycle"), z.literal("dependsOnAncestor"), z.literal("unknownReference"), z.literal("orphanHierarchy")]);

export const ArchiElementSchema = z.strictObject({
	get fqn() { return z.string(); },
	get kind() { return ElementKindSchema; },
	get gate() { return GateTierSchema; },
	get path() { return z.array(z.string()); },
	get summary() { return z.string().nullable(); },
	get metadata() { return z.record(z.string(), z.string()); },
	get modelFile() { return z.string(); },
	get modelLine() { return z.int().min(0).max(4294967295); },
	get mark() { return NodeMarkSchema; },
});

export const ArchiGateChangeSchema = z.strictObject({
	get fqn() { return z.string(); },
	get from() { return GateTierSchema; },
	get to() { return GateTierSchema; },
});

export const ArchiGraphSchema = z.strictObject({
	get elements() { return z.array(ArchiElementSchema); },
	get relations() { return z.array(ArchiRelationSchema); },
	get diagnostics() { return z.array(ArchiDiagnosticSchema); },
});

export const ArchiModuleDetailSchema = z.strictObject({
	get element() { return ArchiElementSchema; },
	get dependsOn() { return z.array(ArchiDependsOnSchema); },
	get dependedBy() { return z.array(ArchiDependedBySchema); },
	get children() { return z.array(z.string()); },
	get docs() { return z.array(DocRefSchema); },
	get warnings() { return z.array(ArchiWarningSchema); },
});

export const ArchiRelationSchema = z.strictObject({
	get from() { return z.string(); },
	get to() { return z.string(); },
	get description() { return z.string().nullable(); },
	get mark() { return EdgeMarkSchema; },
	get reason() { return z.string().nullable(); },
});

export const ArchiWarningSchema = z.strictObject({
	get kind() { return WarningKindSchema; },
	get message() { return z.string(); },
	get element() { return z.string().nullable(); },
	get path() { return z.string().nullable(); },
});

export const ArchiveGroupSchema = z.strictObject({
	get month() { return z.string().nullable(); },
	get changes() { return z.array(ChangeSummarySchema); },
});

export const ArchiveOutcomeSchema = z.strictObject({
	get name() { return z.string(); },
	get archivedDate() { return z.string(); },
});

export const ArchivePreflightSchema = z.strictObject({
	get name() { return z.string(); },
	get completed() { return z.boolean(); },
	get incompletePhases() { return z.array(z.string()); },
	get missingArtifacts() { return z.array(z.string()); },
	get deltaSpecs() { return z.array(z.string()); },
	get worktree() { return z.string().nullable(); },
	get branch() { return z.string().nullable(); },
	get mergeTarget() { return z.string().nullable(); },
});

export const ArtifactDescriptorSchema = z.strictObject({
	get kind() { return z.string(); },
	get source() { return z.string(); },
	get title() { return z.string(); },
	get phase() { return z.string(); },
});

export const ArtifactEnvelopeSchema = z.strictObject({
	get kind() { return z.string(); },
	get version() { return z.int().min(0).max(4294967295); },
	get title() { return z.string(); },
	get payload() { return z.unknown(); },
	get fallbackText() { return z.string().nullable(); },
});

export const AskPayloadSchema = z.strictObject({
	get question() { return z.string(); },
	get options() { return z.array(z.string()); },
});

export const AttemptRecordSchema = z.strictObject({
	get attempt() { return z.int().min(0).max(4294967295).nullable(); },
	get verdict() { return VerdictSchema; },
	get report() { return z.string(); },
	get checklist() { return z.array(ChecklistItemSchema); },
	get skipped() { return z.boolean(); },
	get stale() { return z.boolean(); },
	get startAt() { return z.string().nullable(); },
	get timestamp() { return z.string().nullable(); },
	get backtrackTo() { return z.string().nullable(); },
	get backtrackReason() { return z.string().nullable(); },
	get executorSessionId() { return z.string().nullable(); },
	get evaluatorSessionId() { return z.string().nullable(); },
	get decisionSessionId() { return z.string().nullable(); },
});

export const ChangeDetailSchema = z.strictObject({
	get id() { return z.string(); },
	get name() { return z.string(); },
	get title() { return z.string(); },
	get source() { return ChangeSourceSchema; },
	get status() { return ChangeStatusSchema.nullable(); },
	get created() { return z.string().nullable(); },
	get pipeline() { return z.array(PhaseEntrySchema); },
	get activePhase() { return ActivePhaseSchema.nullable(); },
	get runs() { return z.array(ChangeRunEntrySchema); },
	get artifacts() { return z.array(ArtifactDescriptorSchema); },
	get worktree() { return z.string().nullable(); },
});

export const ChangeDetailUnifiedSchema = z.strictObject({
	get detail() { return ChangeDetailSchema; },
	get activeRun() { return ActiveRunViewSchema.nullable(); },
});

export const ChangeListSchema = z.strictObject({
	get active() { return z.array(ChangeSummarySchema); },
	get archiveGroups() { return z.array(ArchiveGroupSchema); },
});

export const ChangeRunEdgeRecordSchema = z.strictObject({
	get fromNodeKey() { return z.string(); },
	get toNodeKey() { return z.string(); },
	get edgeKind() { return RunEdgeKindSchema; },
});

export const ChangeRunEntrySchema = z.strictObject({
	get runId() { return z.string(); },
	get status() { return RunStatusSchema; },
	get reason() { return z.string().nullable(); },
	get startedAt() { return z.string().nullable(); },
	get finishedAt() { return z.string().nullable(); },
	get nodes() { return z.array(ChangeRunNodeRecordSchema); },
	get edges() { return z.array(ChangeRunEdgeRecordSchema); },
});

export const ChangeRunNodeRecordSchema = z.strictObject({
	get phase() { return z.string(); },
	get attempt() { return z.int().min(0).max(4294967295); },
	get step() { return RunStepKindSchema; },
	get instanceNo() { return z.int().min(0).max(4294967295); },
	get status() { return RunStepStatusSchema; },
	get sessionId() { return z.string().nullable(); },
	get detail() { return z.string().nullable(); },
	get outputRef() { return z.string().nullable(); },
	get startedAt() { return z.number(); },
	get endedAt() { return z.number().nullable(); },
	get tokenInput() { return z.number().nullable(); },
	get tokenOutput() { return z.number().nullable(); },
});

export const ChangeRunStatusSchema = z.union([z.literal("running"), z.literal("waitingConfirm"), z.literal("waitingAsk"), z.literal("completed"), z.literal("stopped"), z.literal("failed")]);

export const ChangeRunSummarySchema = z.strictObject({
	get runId() { return z.string(); },
	get status() { return ChangeRunStatusSchema; },
});

export const ChangeSourceSchema = z.union([z.literal("active"), z.literal("archive")]);

export const ChangeStatusSchema = z.union([z.literal("active"), z.literal("archived")]);

export const ChangeStepKindSchema = z.union([z.literal("executor"), z.literal("evaluator"), z.literal("decision"), z.literal("phaseStart"), z.literal("staticCheck"), z.literal("testExecution"), z.literal("mutationTest"), z.literal("clean"), z.literal("phaseLog"), z.literal("verdictGate"), z.literal("retryGate"), z.literal("whitelistGate"), z.literal("preflight"), z.literal("commit"), z.literal("rebase"), z.literal("resolveConflicts"), z.literal("merge"), z.literal("specSync"), z.literal("cleanup"), z.literal("seal"), z.literal("finalize")]);

export const ChangeStepStateSchema = z.strictObject({
	get phase() { return z.string(); },
	get attempt() { return z.int().min(0).max(4294967295); },
	get step() { return ChangeStepKindSchema; },
	get instanceNo() { return z.int().min(0).max(4294967295); },
	get status() { return ChangeStepStatusSchema; },
	get sessionId() { return z.string().nullable(); },
	get detail() { return z.string().nullable(); },
	get outputRef() { return z.string().nullable(); },
	get startedAt() { return z.number(); },
	get endedAt() { return z.number().nullable(); },
	get tokenInput() { return z.number().nullable(); },
	get tokenOutput() { return z.number().nullable(); },
});

export const ChangeStepStatusSchema = z.union([z.literal("running"), z.literal("passed"), z.literal("failed"), z.literal("stopped")]);

export const ChangeSummarySchema = z.strictObject({
	get id() { return z.string(); },
	get name() { return z.string(); },
	get title() { return z.string(); },
	get source() { return ChangeSourceSchema; },
	get status() { return ChangeStatusSchema.nullable(); },
	get activePhase() { return ActivePhaseSchema.nullable(); },
	get created() { return z.string().nullable(); },
});

export const ChecklistItemSchema = z.strictObject({
	get item() { return z.string(); },
	get pass() { return z.boolean(); },
	get evidence() { return z.string(); },
});

export const CodeStatsReportSchema = z.strictObject({
	get totals() { return CodeTotalsSchema; },
	get languages() { return z.array(LanguageStatsSchema); },
	get tree() { return z.array(TreeEntrySchema); },
});

export const CodeTotalsSchema = z.strictObject({
	get files() { return z.number(); },
	get code() { return z.number(); },
	get comments() { return z.number(); },
	get blanks() { return z.number(); },
});

export const ConfigDiagnosticSchema = z.strictObject({
	get kind() { return DiagnosticKindSchema; },
	get path() { return z.string(); },
	get message() { return z.string(); },
});

export const ConfigExtraFieldSchema = z.strictObject({
	get key() { return z.string(); },
	get value() { return z.unknown(); },
});

export const CoverageThresholdsSchema = z.strictObject({
	get lines() { return z.number().nullable(); },
	get branches() { return z.number().nullable(); },
	get functions() { return z.number().nullable(); },
});

export const CreateOutcomeSchema = z.strictObject({
	get id() { return z.string(); },
	get name() { return z.string(); },
	get created() { return z.string(); },
	get worktree() { return z.string(); },
	get warnings() { return z.array(z.string()); },
});

export const DbDimensionSchema = z.union([z.literal("user"), z.literal("workspace")]);

export const DiagnosticKindSchema = z.union([z.literal("fileMissing"), z.literal("readFailed"), z.literal("jsonInvalid"), z.literal("invalidValue"), z.literal("defaultApplied")]);

export const DirNodeSchema = z.strictObject({
	get name() { return z.string(); },
	get path() { return z.string(); },
	get files() { return z.number(); },
	get code() { return z.number(); },
	get comments() { return z.number(); },
	get blanks() { return z.number(); },
	get children() { return z.array(TreeEntrySchema); },
});

export const DocKindSchema = z.union([z.literal("md"), z.literal("adr")]);

export const DocRefSchema = z.strictObject({
	get path() { return z.string(); },
	get title() { return z.string(); },
	get kind() { return DocKindSchema; },
	get source() { return DocSourceSchema; },
});

export const DocSourceSchema = z.union([z.literal("explicit"), z.literal("adjacent"), z.literal("text")]);

export const EdgeMarkSchema = z.union([z.literal("none"), z.literal("unfulfilled"), z.literal("crossKind"), z.literal("exempt")]);

export const ElementKindSchema = z.union([z.literal("package"), z.literal("domain"), z.literal("module"), z.literal("component")]);

export const ExploreDocSchema = z.strictObject({
	get name() { return z.string(); },
	get content() { return z.string(); },
});

export const ExploreRecordSchema = z.strictObject({
	get id() { return z.number(); },
	get root() { return z.string(); },
	get name() { return z.string(); },
	get title() { return z.string(); },
	get workflowType() { return z.string(); },
	get promotedTo() { return z.string().nullable(); },
	get sessionId() { return z.string().nullable(); },
	get createdAt() { return z.number(); },
	get updatedAt() { return z.number(); },
});

export const ExploreScanEntrySchema = z.strictObject({
	get name() { return z.string(); },
	get modifiedAt() { return z.number().nullable(); },
});

export const FileNodeSchema = z.strictObject({
	get name() { return z.string(); },
	get path() { return z.string(); },
	get code() { return z.number(); },
	get comments() { return z.number(); },
	get blanks() { return z.number(); },
});

export const FileWatchEventSchema = z.strictObject({
	get path() { return z.string(); },
});

export const GateTierSchema = z.union([z.literal("off"), z.literal("warn"), z.literal("strict")]);

export const LanguageStatsSchema = z.strictObject({
	get name() { return z.string(); },
	get files() { return z.number(); },
	get code() { return z.number(); },
	get comments() { return z.number(); },
	get blanks() { return z.number(); },
	get share() { return z.number().nullable(); },
});

export const ModelInfoSchema = z.strictObject({
	get name() { return z.string(); },
	get count() { return z.number(); },
});

export const MutationConfigSchema = z.strictObject({
	get cwd() { return z.string().nullable(); },
	get score() { return z.number().nullable(); },
});

export const NodeMarkSchema = z.union([z.literal("none"), z.literal("unfulfilled")]);

export const PhaseEntrySchema = z.strictObject({
	get phase() { return z.string(); },
	get attempts() { return z.array(AttemptRecordSchema); },
});

export const PromoteOutcomeSchema = z.strictObject({
	get changeId() { return z.string(); },
	get changeName() { return z.string(); },
});

export const RecordEnvelopeSchema = z.strictObject({
	get key() { return z.unknown(); },
	get value() { return z.unknown(); },
});

export const RecordsPageSchema = z.strictObject({
	get records() { return z.array(RecordEnvelopeSchema); },
	get total() { return z.number(); },
});

export const RulesConfigSchema = z.strictObject({
	get proposal() { return z.array(z.string()).nullable(); },
	get tasks() { return z.array(z.string()).nullable(); },
});

export const RunEdgeKindSchema = z.union([z.literal("forward"), z.literal("feedback"), z.literal("retry"), z.literal("backtrack")]);

export const RunNoticeSchema = z.union([z.strictObject({
	get ipc() { return z.literal("step"); },
}), z.strictObject({
	get ipc() { return z.literal("edge"); },
}), z.strictObject({
	get ipc() { return z.literal("sessionEvent"); },
}), z.strictObject({
	get ipc() { return z.literal("ask"); },
}), z.strictObject({
	get ipc() { return z.literal("confirmWait"); },
}), z.strictObject({
	get ipc() { return z.literal("finished"); },
})]);

export const RunStatusSchema = z.union([z.literal("running"), z.literal("completed"), z.literal("stopped"), z.literal("failed"), z.literal("interrupted")]);

export const RunStepKindSchema = z.union([z.literal("executor"), z.literal("evaluator"), z.literal("decision"), z.literal("static_check"), z.literal("test_execution"), z.literal("mutation_test"), z.literal("clean"), z.literal("phase_start"), z.literal("phase_log"), z.literal("verdict_gate"), z.literal("retry_gate"), z.literal("whitelist_gate"), z.literal("preflight"), z.literal("commit"), z.literal("rebase"), z.literal("resolve_conflicts"), z.literal("merge"), z.literal("spec_sync"), z.literal("cleanup"), z.literal("seal"), z.literal("finalize")]);

export const RunStepStatusSchema = z.union([z.literal("running"), z.literal("passed"), z.literal("failed"), z.literal("stopped")]);

export const SessionPageSchema = z.strictObject({
	get sessions() { return z.array(SessionSummarySchema); },
	get total() { return z.number(); },
});

export const SessionProvenanceSchema = z.strictObject({
	get source() { return z.string(); },
	get sourceRef() { return z.string().nullable(); },
});

export const SessionRowSchema = z.strictObject({
	get id() { return z.string(); },
	get remoteSessionId() { return z.string().nullable(); },
	get configSnapshot() { return z.unknown(); },
	get provenance() { return SessionProvenanceSchema; },
	get createdAt() { return z.number(); },
	get updatedAt() { return z.number(); },
});

export const SessionStatsSchema = z.strictObject({
	get turnCount() { return z.number(); },
	get totalDurationMs() { return z.number().nullable(); },
	get inputTokens() { return z.number().nullable(); },
	get outputTokens() { return z.number().nullable(); },
});

export const SessionSummarySchema = z.strictObject({
	get row() { return SessionRowSchema; },
	get stats() { return SessionStatsSchema; },
	get turns() { return z.array(TurnSummarySchema); },
});

export const TestFrameworkSchema = z.union([z.literal("jest"), z.literal("vitest"), z.literal("vite-plus"), z.literal("bun"), z.literal("rust"), z.literal("node-test"), z.literal("go"), z.literal("pytest")]);

export const TestSuiteSchema = z.strictObject({
	get root() { return z.string(); },
	get framework() { return TestFrameworkSchema; },
	get cwd() { return z.string(); },
	get config() { return z.string().nullable(); },
	get includes() { return z.array(z.string()).nullable(); },
	get excludes() { return z.array(z.string()).nullable(); },
	get coverage() { return CoverageThresholdsSchema; },
	get mutation() { return MutationConfigSchema; },
});

export const TreeEntrySchema = z.union([z.strictObject({
	get kind() { return z.literal("dir"); },
	get node() { return DirNodeSchema; },
}), z.strictObject({
	get kind() { return z.literal("file"); },
	get node() { return FileNodeSchema; },
})]);

export const TurnSummarySchema = z.strictObject({
	get turnId() { return z.number(); },
	get sessionId() { return z.string(); },
	get status() { return AgentRunStatusSchema; },
	get startedAt() { return z.number(); },
	get finishedAt() { return z.number().nullable(); },
	get numTurns() { return z.number().nullable(); },
	get costUsd() { return z.number().nullable().nullable(); },
	get durationMs() { return z.number().nullable(); },
	get error() { return z.string().nullable(); },
});

export const VerdictSchema = z.union([z.literal("pass"), z.literal("fail")]);

export const WarningKindSchema = z.literal("docNotFound");

export const WebSearchSourceSchema = z.strictObject({
	get url() { return z.string(); },
	get title() { return z.string().nullable().optional(); },
	get pageAge() { return z.string().nullable().optional(); },
});

export const WorkspaceConfigSchema = z.strictObject({
	get $schema() { return z.string().nullable(); },
	get schema() { return z.string(); },
	get context() { return z.string().nullable(); },
	get rules() { return RulesConfigSchema.nullable(); },
	get staticAnalysis() { return z.string().nullable(); },
	get tests() { return z.array(TestSuiteSchema); },
	get writeProtection() { return WriteProtectionSchema.nullable(); },
	get extra() { return z.array(ConfigExtraFieldSchema); },
});

export const WorkspaceConfigReportSchema = z.strictObject({
	get config() { return WorkspaceConfigSchema; },
	get diagnostics() { return z.array(ConfigDiagnosticSchema); },
});

export const WorkspaceRecordSchema = z.strictObject({
	get root() { return z.string(); },
	get name() { return z.string(); },
	get addedAt() { return z.number(); },
});

export const WriteProtectionSchema = z.strictObject({
	get files() { return z.array(WriteProtectionFileSchema).nullable(); },
});

export const WriteProtectionFileSchema = z.strictObject({
	get glob() { return z.string().nullable(); },
	get reason() { return z.string().nullable(); },
});
