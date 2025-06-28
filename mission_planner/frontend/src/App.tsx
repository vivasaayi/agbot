import React from 'react';
import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import MissionPlanner from './components/MissionPlanner';
import './App.css';

function App() {
  return (
    <Router>
      <div className="App">
        <Routes>
          <Route path="/" element={<MissionPlanner />} />
        </Routes>
      </div>
    </Router>
  );
}

export default App;
