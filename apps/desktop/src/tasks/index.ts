// src/tasks: bise's tasks on the desktop (brief tasks-build, page tasks-center): the right bar's
// Tasks section, the Board and the Task tab (panel tabs), the status mark and the task chip. The
// tasks are the hub's (HubEv works); their line's rule is what.ts. Owner: tasks.
export { FloatTasks, OuterTasks, lineAt } from './Side';
export { BoardTab, useTaskAct } from './Board';
export { TaskTab } from './TaskTab';
export { openBoard, openTask, statusWord, TaskChip, TaskChips, TaskMark } from './Marks';
export { useAllWorks } from './hooks';
export { chipOrder, TaskChipsOf, tasksOf, type ChipsOf } from './chips';
export { barRows, cutAtWord, goalsOf, isOpen, needsHim, STATUS_ORDER, type ProjWork } from './model';
export { whatOf } from './what';
