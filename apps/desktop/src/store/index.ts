// '@/store': the one store of bise desktop and its hooks (owner: amb-core).
// Read: useProjects, useAgents, useCards, useThreadOrder, useEntry, useTyping,
// usePending, ... Act: useStore().do(intent: a card's answer, close, retry; machine/types.ts) / send / stop / subscribeThread /
// olderPage / files (useFiles), askSkills / dictateStart, dictateStop, dictateCancel (useDictation) / voiceMode, voiceMute, voiceType, voiceSend, voiceCut (useVoiceMode) /
// command(any typed AppCmd | HubCmd). Tests: new Store(fakePort),
// StoreProvider, replay(store, fixtures) from './replay'.
export { prDiffKey, rangeDiffKey, type PrActState } from './types';
export { Store, MARKS, TAGS, typed, threadKey, diffKey, branchDiffKey, takeKey, toolOutKey, type QueuedTake, type ToolOut, type Ev, type EvOf, type Pending, type ThreadHits, type ThreadMarks, type ThreadOrder, type Diff, type GuideEv, type Away, type Dictation, type HubLine, type FilesAnswer, type PrsList, type InboxList, type BranchesList, type VersionsList, type PagesList, type VoiceModeState } from './store';
export * from './hooks';
export type { AnswerAt, AnswerFrom, AnsweredFrom } from './answered';
export type { AnswerJump } from './store';
export { actedKeys, type Acted } from './acted';
export { refusedLine, type Refused } from './refused';
export { replay, allFixtureEvents } from './replay';
