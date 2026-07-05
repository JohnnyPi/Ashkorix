import {
    createContext,
    useCallback,
    useContext,
    useMemo,
    useRef,
    useState,
    type ReactNode,
  } from "react";
  
  export type ChatSessionInfo = {
    /** Number of user turns in the live conversation. */
    turns: number;
    /** Whether the model is currently streaming a response. */
    streaming: boolean;
    /** Id of the saved session currently loaded, or null for a fresh chat. */
    activeSessionId: string | null;
  };
  
  export type ChatSessionActions = {
    /** Archive the current transcript and start a fresh session. */
    newSession: () => void | Promise<void>;
    /** Export the current conversation. */
    exportChat: () => void | Promise<void>;
    /** Archive the current chat, then load a saved session into the conversation. */
    loadSession: (sessionId: string) => void | Promise<void>;
  };
  
  type ChatSessionContextValue = {
    session: ChatSessionInfo;
    actions: ChatSessionActions;
    /** Bumped whenever the saved-session list may have changed; consumers re-fetch. */
    sessionsRevision: number;
    /** Called by ChatPage to publish live session state to other consumers. */
    publish: (info: ChatSessionInfo) => void;
    /** Called by ChatPage to register the handlers that drive chat actions. */
    registerActions: (actions: ChatSessionActions) => void;
    /** Trigger a refresh of the saved-session list. */
    refreshSessions: () => void;
  };
  
  const NOOP_ACTIONS: ChatSessionActions = {
    newSession: () => {},
    exportChat: () => {},
    loadSession: () => {},
  };
  
  const ChatSessionContext = createContext<ChatSessionContextValue | null>(null);
  
  export function ChatSessionProvider({ children }: { children: ReactNode }) {
    const [session, setSession] = useState<ChatSessionInfo>({
      turns: 0,
      streaming: false,
      activeSessionId: null,
    });
    const [sessionsRevision, setSessionsRevision] = useState(0);
    const actionsRef = useRef<ChatSessionActions>(NOOP_ACTIONS);
  
    // Dedupe so a no-op publish doesn't re-render consumers.
    const publish = useCallback((info: ChatSessionInfo) => {
      setSession((prev) =>
        prev.turns === info.turns &&
        prev.streaming === info.streaming &&
        prev.activeSessionId === info.activeSessionId
          ? prev
          : info,
      );
    }, []);
  
    const registerActions = useCallback((next: ChatSessionActions) => {
      actionsRef.current = next;
    }, []);
  
    const refreshSessions = useCallback(() => setSessionsRevision((r) => r + 1), []);
  
    // Stable wrappers that always delegate to the latest registered handler, so
    // consumers never hold a stale closure and re-registration causes no re-render.
    const actions = useMemo<ChatSessionActions>(
      () => ({
        newSession: () => actionsRef.current.newSession(),
        exportChat: () => actionsRef.current.exportChat(),
        loadSession: (id: string) => actionsRef.current.loadSession(id),
      }),
      [],
    );
  
    const value = useMemo(
      () => ({ session, actions, sessionsRevision, publish, registerActions, refreshSessions }),
      [session, actions, sessionsRevision, publish, registerActions, refreshSessions],
    );
  
    return <ChatSessionContext.Provider value={value}>{children}</ChatSessionContext.Provider>;
  }
  
  export function useChatSession(): ChatSessionContextValue {
    const ctx = useContext(ChatSessionContext);
    if (!ctx) {
      throw new Error("useChatSession must be used within ChatSessionProvider");
    }
    return ctx;
  }