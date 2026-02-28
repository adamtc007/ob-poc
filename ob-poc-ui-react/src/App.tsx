import { QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { queryClient } from "./lib/query";
import { AppShell, ErrorBoundary } from "./components";
import { ChatPage } from "./features/chat/ChatPage";
import { SemOsPage } from "./features/semantic-os/SemOsPage";
import { DealPage } from "./features/deal/DealPage";
import { InspectorPage } from "./features/inspector/InspectorPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { ViewportPage } from "./features/viewport/ViewportPage";

function App() {
  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Routes>
            {/* Pop-out viewport window (no AppShell) */}
            <Route
              path="viewport/:sessionId"
              element={
                <ErrorBoundary>
                  <ViewportPage />
                </ErrorBoundary>
              }
            />

            {/* Main app with navigation shell */}
            <Route path="/" element={<AppShell />}>
              <Route index element={<Navigate to="/chat" replace />} />
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
                path="semantic-os"
                element={
                  <ErrorBoundary>
                    <SemOsPage />
                  </ErrorBoundary>
                }
              />
              <Route
                path="semantic-os/:sessionId"
                element={
                  <ErrorBoundary>
                    <SemOsPage />
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
              <Route
                path="deal/:dealId"
                element={
                  <ErrorBoundary>
                    <DealPage />
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
