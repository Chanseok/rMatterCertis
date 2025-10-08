import { Router, Route } from "@solidjs/router";
import { LocalDbDashboard } from "./components/LocalDbDashboard";

function App() {
  return (
    <Router>
      <Route path="/crawl" component={LocalDbDashboard} />
    </Router>
  );
}

export default App;