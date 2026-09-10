import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom"
import { DashboardLayout } from "@/components/layout/dashboard-layout"
import { Toaster } from "@/components/ui/sonner"
import { getToken } from "@/lib/api"
import ForgotPage from "@/pages/forgot"
import HelpPage from "@/pages/help"
import LoginPage from "@/pages/login"
import ProjectPage from "@/pages/project"
import ProjectsPage from "@/pages/projects"
import RegisterPage from "@/pages/register"
import SettingsPage from "@/pages/settings"
import VerifyPage from "@/pages/verify"
import CliLoginPage from "@/pages/cli"

function RequireAuth() {
  if (!getToken()) return <Navigate to="/login" replace />
  return <DashboardLayout />
}

export default function App() {
  return (
    <BrowserRouter>
      <Toaster position="top-center" richColors />
      <Routes>
        <Route path="login" element={<LoginPage />} />
        <Route path="forgot" element={<ForgotPage />} />
        <Route path="register" element={<RegisterPage />} />
        <Route path="verify" element={<VerifyPage />} />
        <Route path="help" element={<HelpPage />} />
        <Route path="cli" element={<CliLoginPage />} />
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
