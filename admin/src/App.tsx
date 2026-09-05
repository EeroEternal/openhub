import { BrowserRouter, Navigate, Outlet, Route, Routes } from "react-router-dom"
import { DashboardLayout } from "@/components/layout/dashboard-layout"
import { Toaster } from "@/components/ui/sonner"
import { getToken } from "@/lib/api"
import LoginPage from "@/pages/login"
import ProjectPage from "@/pages/project"
import ProjectsPage from "@/pages/projects"
import RegisterPage from "@/pages/register"
import SettingsPage from "@/pages/settings"
import VerifyPage from "@/pages/verify"

function RequireAuth() {
  if (!getToken()) return <Navigate to="/login" replace />
  return <DashboardLayout />
}

function PublicOnly() {
  if (getToken()) return <Navigate to="/" replace />
  return <Outlet />
}

export default function App() {
  return (
    <BrowserRouter>
      <Toaster position="top-center" richColors />
      <Routes>
        <Route element={<PublicOnly />}>
          <Route path="login" element={<LoginPage />} />
          <Route path="register" element={<RegisterPage />} />
          <Route path="verify" element={<VerifyPage />} />
        </Route>
        <Route element={<RequireAuth />}>
          <Route index element={<ProjectsPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="projects/:id" element={<ProjectPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}
