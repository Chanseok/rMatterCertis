import { Router, Route } from "@solidjs/router";
import { LocalDbDashboard } from "./components/LocalDbDashboard";
import { DatabaseDiagnostics } from "./components/DatabaseDiagnostics";

function App() {
  return (
    <Router>
      <Route path="/crawl" component={LocalDbDashboard} />
      <Route path="/diagnostics" component={DatabaseDiagnostics} />
    </Router>
  );
}

export default App;