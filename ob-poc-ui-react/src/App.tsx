import { QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { queryClient } from "./lib/query";
import { AppShell, ErrorBoundary } from "./components";
import { ChatPage } from "./features/chat/ChatPage";
import { DealPage } from "./features/deal/DealPage";
import { InspectorPage } from "./features/inspector/InspectorPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { ViewportPage } from "./features/viewport/ViewportPage";
import { ObservatoryPage } from "./features/observatory/ObservatoryPage";
import { CataloguePage } from "./features/catalogue/CataloguePage";

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
            {/* Chat routes — full viewport cockpit, no AppShell nav */}
            <Route index element={<Navigate to="/chat" replace />} />
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
            <Route path="semantic-os" element={<Navigate to="/chat" replace />} />
            <Route path="semantic-os/:sessionId" element={<Navigate to="/chat" replace />} />

            {/* Observatory full-screen option */}
            <Route
              path="observatory/:sessionId"
              element={
                <ErrorBoundary>
                  <ObservatoryPage />
                </ErrorBoundary>
              }
            />

            {/* Secondary views — keep AppShell nav */}
            <Route path="/" element={<AppShell />}>
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
                path="settings"
                element={
                  <ErrorBoundary>
                    <SettingsPage />
                  </ErrorBoundary>
                }
              />
              {/* Tranche 3 Phase 3.D / Observatory Phase 8 — Catalogue workspace
                  read-only panel: pending proposals + diff + tier heatmap. */}
              <Route
                path="catalogue"
                element={
                  <ErrorBoundary>
                    <CataloguePage />
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
