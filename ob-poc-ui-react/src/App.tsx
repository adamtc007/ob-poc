import { QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { queryClient } from "./lib/query";
import { AppShell, ErrorBoundary } from "./components";
import { InspectorPage } from "./features/inspector/InspectorPage";
import { ChatPage } from "./features/chat/ChatPage";
import { SettingsPage } from "./features/settings/SettingsPage";

function App() {
  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Routes>
            <Route path="/" element={<AppShell />}>
              <Route index element={<Navigate to="/inspector" replace />} />
              <Route
                path="inspector"
                element={
                  <ErrorBoundary>
                    <InspectorPage />
                  </ErrorBoundary>
                }
              />
              <Route
                path="inspector/:projectionId"
                element={
                  <ErrorBoundary>
                    <InspectorPage />
                  </ErrorBoundary>
                }
              />
              <Route
                path="chat"
                element={
                  <ErrorBoundary>
                    <ChatPage />
                  </ErrorBoundary>
                }
              />
              <Route
                path="chat/:sessionId"
                element={
                  <ErrorBoundary>
                    <ChatPage />
                  </ErrorBoundary>
                }
              />
              <Route
                path="settings"
                element={
                  <ErrorBoundary>
                    <SettingsPage />
                  </ErrorBoundary>
                }
              />
            </Route>
          </Routes>
        </BrowserRouter>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export default App;
