import type {
  Topic,
  TopicId,
  Environment,
  EnvId,
  Rebuild,
  Conflict,
} from "../../../generated/types.js";

/** Lightweight header for a kanban lane — only the fields LaneColumn reads. */
export interface LaneHeader {
  readonly id: string;
  readonly name: string;
  readonly branch: string;
}

export interface Lane {
  header: LaneHeader;
  env: Environment | null;
  topics: Topic[];
  totalInEnv: number;
  lastRebuild: Rebuild | undefined;
}

export interface LaneColumnProps {
  header: LaneHeader;
  isUnassigned: boolean;
  topics: Topic[];
  totalInEnv: number;
  lastRebuild: Rebuild | undefined;
  topicConflictsMap: Map<TopicId, Conflict[]>;
  isCurrentLane: boolean;
  focusedCardIndex: number | null;
  selectedTopicId: TopicId | null;
  nextEnv: Environment | undefined;
  isMutating: boolean;
  isLastEnvLane: boolean;
  allEnvs: Environment[];
  topicEnvMap: Map<TopicId, Set<EnvId>>;
  topicHighestEnv: Map<TopicId, EnvId>;
  onCardRef: (id: TopicId, el: HTMLDivElement | null) => void;
  onScrollRef: (envId: string, el: HTMLDivElement | null) => void;
  onScroll: (envId: string) => void;
  onSelect: (topic: Topic, cardIndex: number) => void;
  onPromote: (topicId: TopicId, repoId: string) => void;
  onRebuild: () => void;
  onGraduate: (topic: Topic) => void;
  onClose: (topicId: TopicId, repoId: string) => void;
}

export interface UnassignedLaneContentProps {
  topics: Topic[];
  topicConflictsMap: Map<TopicId, Conflict[]>;
  isCurrentLane: boolean;
  focusedCardIndex: number | null;
  selectedTopicId: TopicId | null;
  isMutating: boolean;
  allEnvs: Environment[];
  topicEnvMap: Map<TopicId, Set<EnvId>>;
  topicHighestEnv: Map<TopicId, EnvId>;
  firstEnv: Environment | undefined;
  onCardRef: (id: TopicId, el: HTMLDivElement | null) => void;
  onSelect: (topic: Topic, cardIndex: number) => void;
  onPromote: (topicId: TopicId, repoId: string) => void;
  onClose: (topicId: TopicId, repoId: string) => void;
}

export interface TopicCardProps {
  topic: Topic;
  conflicts: Conflict[];
  cardIndex: number;
  isFocused: boolean;
  isSelected: boolean;
  nextEnv: Environment | undefined;
  isUnassignedLane: boolean;
  isLastEnvLane: boolean;
  isMutating: boolean;
  allEnvs: Environment[];
  topicEnvIds: Set<EnvId>;
  highestEnvId: EnvId | null;
  onRef: (id: TopicId, el: HTMLDivElement | null) => void;
  onSelect: (topic: Topic, cardIndex: number) => void;
  onPromote: (topicId: TopicId, repoId: string) => void;
  onGraduate: (topic: Topic) => void;
}

export interface PromotionTrailProps {
  allEnvs: Environment[];
  topicEnvIds: Set<EnvId>;
  highestEnvId: EnvId | null;
}
