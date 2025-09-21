import React from "react";
import ReactDOM from "react-dom/client";
import { createBrowserRouter, RouterProvider } from "react-router-dom";
import App from "./App";
import Home from "./components/Home";
import Catalogue from "./components/Catalogue";
import GameDetail from "./components/GameDetail";
import Settings from "./components/Settings";
import UserProfile from "./components/UserProfile";
import Bypass from "./components/Bypass";
import "./App.css";

const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: <Home /> },
      { path: "catalogue", element: <Catalogue /> },
      { path: "bypass", element: <Bypass /> },
      { path: "settings", element: <Settings /> },
      { path: "profile", element: <UserProfile /> },
      { path: "game/:appId", element: <GameDetail /> },
    ],
  },
]);

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <RouterProvider router={router} />
  </React.StrictMode>
);
