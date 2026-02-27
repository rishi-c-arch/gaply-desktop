import React, { useState, useEffect, useRef, useMemo } from 'react';
import SEO from './SEO';
import { apiFetch } from '../api/config';
import { enhanceFormalReportHTML } from '../utils/formalResearchReportEnhance';
import * as XLSX from 'xlsx';
import './StatisticalResearchOrchestratorPage.css';

type Variable = {
  name: string;
  type: 'nominal' | 'ordinal' | 'interval' | 'ratio';
  role: 'iv' | 'dv' | 'covariate' | 'id';
};

type ParsedTable = {
  table_id: string;
  source_file: string;
  sheet_name: string | null;
  n_rows: number;
  n_columns: number;
  columns: Array<{ name: string; detected_type: string; notes?: string | null }>;
  sample_rows: Array<Record<string, any>>;
  parsing_warnings: string[];
};

type AnalysisOutput = {
  test_id: string;
  test_name: string;
  input_columns: string[];
  params: Record<string, any>;
  numeric_results: {
    statistic: number | null;
    df: number | null;
    p_value: number | null;
    effect_size: number | null;
    ci: [number, number] | null;
    notes: string | null;
  };
  table_html: string | null;
  plot_url: string | null;
};

// Full taxonomy of analysis tools (Universal, Quantitative, Qualitative, Mixed, Computational, Domain, Causal, Advanced, Reporting).
// Backend may implement a subset; unknown keys are sent as selected_modules and can be ignored or mapped.
const TEST_MODULES_BY_CATEGORY: { category: string; modules: { key: string; label: string; desc: string }[] }[] = [
  {
    category: 'Core analyses',
    modules: [
      { key: 'data_preparation', label: 'Data Preparation & Screening', desc: 'Data cleaning, missing value analysis, outlier detection, transformations, coding & recoding, reverse coding, scale scores.' },
      { key: 'descriptive_stats', label: 'Descriptive Statistics', desc: 'Frequency, percentage, mean, median, mode, min/max, range, variance, SD, IQR, skewness, kurtosis, cross-tabs.' },
      { key: 'assumption_tests', label: 'Assumption Testing', desc: 'Shapiro–Wilk, KS, Anderson–Darling, Q–Q, histogram, Levene, Bartlett, Box’s M, scatter, Durbin–Watson.' },
      { key: 'reliability', label: 'Reliability Analysis', desc: 'Cronbach’s alpha, McDonald’s omega, split-half, composite reliability, item-total, alpha if deleted.' },
      { key: 'validity', label: 'Validity Analysis', desc: 'CVI, EFA, CFA, AVE, Fornell–Larcker, HTMT ratio.' },
      { key: 'parametric_tests', label: 'Parametric Tests', desc: 'One-sample, independent, paired t-tests; one-way/two-way/repeated ANOVA; MANOVA/MANCOVA/ANCOVA; post-hoc.' },
      { key: 'non_parametric_tests', label: 'Non-Parametric Tests', desc: 'Mann–Whitney, Wilcoxon, Kruskal–Wallis, Friedman, Chi-Square, Fisher’s Exact, McNemar.' },
      { key: 'association', label: 'Association & Relationship', desc: 'Pearson, Spearman, Kendall’s tau, point-biserial, partial, Cohen’s kappa, Fleiss’ kappa, ICC.' },
      { key: 'regression', label: 'Regression Analysis', desc: 'Simple/multiple linear, logistic, hierarchical, stepwise, polynomial, ridge, lasso, diagnostics.' },
      { key: 'mediation', label: 'Mediation & Moderation', desc: 'Baron & Kenny, bootstrapped mediation, moderation, moderated mediation, PROCESS-style.' },
      { key: 'sem', label: 'SEM / Path Analysis', desc: 'CB-SEM, PLS-SEM, measurement/structural models, fit indices (χ², RMSEA, CFI, TLI, SRMR).' },
      { key: 'multivariate', label: 'Multivariate Analysis', desc: 'PCA, factor analysis, discriminant, canonical correlation, cluster analysis, MDS.' },
      { key: 'time_series', label: 'Time Series & Longitudinal', desc: 'Trend, seasonal decomposition, ACF/PACF, ARIMA/SARIMA, panel data.' },
      { key: 'survival', label: 'Survival & Event Analysis', desc: 'Kaplan–Meier, log-rank, Cox proportional hazards.' },
      { key: 'experimental', label: 'Experimental / Design-Based', desc: 'RCT analysis, pre/post design, factorial, Latin square, mixed-design ANOVA.' },
      { key: 'effect_power', label: 'Effect Size & Power', desc: 'Cohen’s d, eta squared, partial eta squared, odds ratio, relative risk, power analysis.' },
      { key: 'ml_analytics', label: 'ML & Advanced Analytics', desc: 'Classification, regression trees, neural networks, clustering, feature importance.' },
      { key: 'qual_mixed', label: 'Qualitative / Mixed Methods', desc: 'Thematic, content analysis, grounded theory coding, sentiment, NVivo/Atlas.ti metrics.' },
      { key: 'domain_addons', label: 'Domain-Specific Add-ons', desc: 'SERVQUAL, balanced scorecard, IRT/Rasch, legal text analysis, DOE/response surface.' },
      { key: 'reporting_integrity', label: 'Reporting & Integrity Checks', desc: 'Assumption verification, diagnostics, robustness, sensitivity, reproducibility.' },
    ],
  },
  {
    category: 'Universal / study-level',
    modules: [
      { key: 'literature_review', label: 'Literature / systematic review', desc: 'Map prior work, gaps, and theoretical framing.' },
      { key: 'protocol_prereg', label: 'Protocol design & pre-registration', desc: 'Define hypotheses, methods, and analysis plan before data collection.' },
      { key: 'power_analysis', label: 'Sample size / power analysis', desc: 'Calculate needed sample to detect effects.' },
      { key: 'ethics_irb', label: 'Ethics & IRB review', desc: 'Assess participant risk, consent, data handling.' },
      { key: 'data_cleaning_preprocessing', label: 'Data cleaning & preprocessing', desc: 'Remove errors, recode variables, standardize formats.' },
      { key: 'missing_data_imputation', label: 'Missing data analysis & imputation', desc: 'Assess missingness mechanism and impute (e.g., multiple imputation).' },
      { key: 'eda', label: 'Exploratory data analysis (EDA)', desc: 'Visualize distributions, detect outliers, check assumptions.' },
      { key: 'sensitivity_robustness', label: 'Sensitivity / robustness checks', desc: 'Test whether results hold under alternate specifications.' },
      { key: 'model_diagnostics', label: 'Model diagnostics & validation', desc: 'Residuals, multicollinearity, heteroskedasticity, calibration.' },
      { key: 'multiple_comparisons', label: 'Multiple comparisons correction', desc: 'Control false discovery (e.g., Bonferroni, FDR).' },
      { key: 'reproducibility', label: 'Reproducibility checks & replication', desc: 'Share code/data, rerun analyses, reproduce results.' },
      { key: 'reporting_visualization', label: 'Reporting & visualization', desc: 'Tables, plots, reproducible reports (e.g., R Markdown).' },
      { key: 'data_management_metadata', label: 'Data management & metadata', desc: 'Document datasets, variable definitions, provenance.' },
    ],
  },
  {
    category: 'Quantitative statistical',
    modules: [
      { key: 't_tests_nonparametric', label: 't-tests & nonparametric equivalents', desc: 'Compare two groups (Mann–Whitney, Wilcoxon).' },
      { key: 'anova_manova', label: 'ANOVA / MANOVA', desc: 'Compare means across >2 groups; multivariate outcomes.' },
      { key: 'regression_family', label: 'Regression (linear, logistic, Poisson, etc.)', desc: 'Model relationships between predictors and outcomes.' },
      { key: 'glm', label: 'Generalized linear models (GLM)', desc: 'Flexible regression family for various data types.' },
      { key: 'mixed_multilevel', label: 'Mixed-effects / multilevel models', desc: 'Nested or hierarchical data (e.g., students within schools).' },
      { key: 'time_series_arima', label: 'Time-series analysis', desc: 'ARIMA, seasonal decomposition, forecasting.' },
      { key: 'panel_longitudinal', label: 'Panel data / longitudinal analysis', desc: 'Fixed/random effects, growth curve models.' },
      { key: 'survival_time_to_event', label: 'Survival / time-to-event analysis', desc: 'Kaplan–Meier, Cox proportional hazards.' },
      { key: 'factor_pca', label: 'Factor analysis & PCA', desc: 'Reduce dimensionality, find latent constructs.' },
      { key: 'sem_latent', label: 'Structural equation modeling (SEM)', desc: 'Complex causal and latent-variable models.' },
      { key: 'latent_class_profile', label: 'Latent class / latent profile analysis', desc: 'Identify unobserved subgroups.' },
      { key: 'cluster_analysis', label: 'Cluster analysis', desc: 'k-means, hierarchical clustering for grouping observations.' },
      { key: 'bayesian', label: 'Bayesian analysis', desc: 'Posterior estimation, hierarchical Bayesian models.' },
      { key: 'bootstrap_permutation', label: 'Bootstrap & permutation tests', desc: 'Nonparametric inference and CI estimation.' },
      { key: 'meta_analysis', label: 'Meta-analysis & meta-regression', desc: 'Pool effect sizes across studies.' },
      { key: 'econometric_causal', label: 'Econometric causal methods', desc: 'IV, difference-in-differences (DiD), regression discontinuity (RDD).' },
      { key: 'propensity_score', label: 'Propensity score methods', desc: 'Matching, weighting for observational causal inference.' },
      { key: 'power_curves_posthoc', label: 'Power curves & post-hoc power analysis', desc: 'Assess detectable effect sizes.' },
    ],
  },
  {
    category: 'Qualitative analyses',
    modules: [
      { key: 'thematic_analysis', label: 'Thematic analysis', desc: 'Identify themes across interview/focus-group data.' },
      { key: 'grounded_theory', label: 'Grounded theory', desc: 'Iterative coding to generate theory from data.' },
      { key: 'content_analysis', label: 'Content analysis (quantitative/qualitative)', desc: 'Code texts for frequency and meaning.' },
      { key: 'discourse_analysis', label: 'Discourse analysis', desc: 'Study language use, power, and ideology in texts.' },
      { key: 'narrative_analysis', label: 'Narrative analysis', desc: 'Analyze stories and individual accounts.' },
      { key: 'framework_analysis', label: 'Framework analysis', desc: 'Matrix-based method for applied qualitative projects.' },
      { key: 'phenomenological', label: 'Phenomenological analysis', desc: 'Describe lived experiences and meanings.' },
      { key: 'qca', label: 'Qualitative comparative analysis (QCA)', desc: 'Set-theoretic method bridging qual & quant.' },
      { key: 'codebook_intercoder', label: 'Codebook development & intercoder reliability', desc: 'Create codes and measure coder agreement (e.g., Krippendorff’s α).' },
    ],
  },
  {
    category: 'Mixed-methods',
    modules: [
      { key: 'convergent_parallel', label: 'Convergent parallel design analysis', desc: 'Analyze qual & quant separately then integrate.' },
      { key: 'explanatory_sequential', label: 'Explanatory sequential analysis', desc: 'Use qualitative to explain quantitative results (or vice versa).' },
      { key: 'triangulation', label: 'Triangulation', desc: 'Cross-validate findings from multiple methods.' },
    ],
  },
  {
    category: 'Computational & data-science',
    modules: [
      { key: 'ml_supervised', label: 'Machine learning (supervised)', desc: 'Classification/regression: random forest, XGBoost, SVM.' },
      { key: 'ml_unsupervised', label: 'Machine learning (unsupervised)', desc: 'Clustering, dimensionality reduction, anomaly detection.' },
      { key: 'deep_learning', label: 'Deep learning / neural networks', desc: 'Image, audio, or complex tabular/text modeling.' },
      { key: 'nlp', label: 'Natural language processing (NLP)', desc: 'Tokenization, topic modeling (LDA), transformer models.' },
      { key: 'topic_modeling_semantic', label: 'Topic modeling & semantic analysis', desc: 'Discover themes across large corpora.' },
      { key: 'sentiment_analysis', label: 'Sentiment analysis', desc: 'Measure opinion polarity in text data.' },
      { key: 'network_sna', label: 'Network / social network analysis (SNA)', desc: 'Nodes/edges metrics, community detection, centrality.' },
      { key: 'graph_analysis', label: 'Graph analysis & link prediction', desc: 'Study relational structures.' },
      { key: 'abm', label: 'Agent-based modeling (ABM)', desc: 'Simulate interactions of autonomous agents.' },
      { key: 'simulation_monte_carlo', label: 'Simulation & Monte Carlo studies', desc: 'Evaluate estimators, system behavior under scenarios.' },
      { key: 'image_video_vision', label: 'Image/video analysis (computer vision)', desc: 'Object detection, segmentation, feature extraction.' },
      { key: 'spatial_geospatial', label: 'Spatial / geospatial analysis', desc: 'GIS mapping, spatial statistics, hotspot analysis.' },
      { key: 'hpc_parallel', label: 'High-performance & parallel computing', desc: 'Big-data workflows, distributed processing.' },
    ],
  },
  {
    category: 'Domain-specific (selected)',
    modules: [
      { key: 'domain_economics', label: 'Economics', desc: 'Time-series econometrics, CGE models, input–output, production functions, DiD.' },
      { key: 'domain_psychology', label: 'Psychology', desc: 'Psychometrics, IRT/Rasch, CFA, experimental ANOVA.' },
      { key: 'domain_education', label: 'Education', desc: 'Item analysis, DIF, multilevel growth models, programme evaluation.' },
      { key: 'domain_medicine_ph', label: 'Medicine & Public Health', desc: 'RCT analysis, ITT, subgroup analysis, meta-analysis, ROC.' },
      { key: 'domain_epidemiology', label: 'Epidemiology', desc: 'Incidence/prevalence, case–control, cohort survival, outbreak modeling.' },
      { key: 'domain_biology_genomics', label: 'Biology / Genomics', desc: 'Differential gene expression, sequence alignment, phylogenetic inference, GWAS.' },
      { key: 'domain_neuroscience', label: 'Neuroscience', desc: 'fMRI preprocessing, GLM for brain imaging, connectivity, EEG spectral analysis.' },
      { key: 'domain_environmental', label: 'Environmental Science', desc: 'Climate trends, species distribution, remote sensing, water quality modeling.' },
      { key: 'domain_geography_urban', label: 'Geography / Urban Planning', desc: 'Spatial econometrics, accessibility, transport simulation, land-use change.' },
      { key: 'domain_engineering', label: 'Engineering', desc: 'FEA, CFD, reliability analysis, system identification.' },
      { key: 'domain_cs_se', label: 'Computer Science / SE', desc: 'Algorithmic complexity, software metrics, A/B testing.' },
      { key: 'domain_law', label: 'Law & Legal Studies', desc: 'Doctrinal analysis, comparative legal, case-citation networks, policy impact.' },
      { key: 'domain_business', label: 'Business & Management', desc: 'Case-study synthesis, PLS-SEM, conjoint analysis, customer segmentation.' },
      { key: 'domain_marketing', label: 'Marketing', desc: 'Marketing-mix modeling, conjoint, choice modeling, sentiment/trend analysis.' },
      { key: 'domain_finance', label: 'Finance', desc: 'Event study, GARCH, VaR, portfolio optimization.' },
      { key: 'domain_linguistics', label: 'Linguistics', desc: 'Phonetic/phonological analysis, corpus linguistics, syntactic parsing.' },
      { key: 'domain_anthropology', label: 'Anthropology', desc: 'Ethnographic coding, kinship/network mapping, material culture analysis.' },
      { key: 'domain_agriculture', label: 'Agriculture / Field Trials', desc: 'RCBD, split-plot ANOVA, agronomic modeling.' },
      { key: 'domain_materials_chemistry', label: 'Materials / Chemistry', desc: 'Spectral deconvolution, crystallographic analysis, kinetics, DFT.' },
    ],
  },
  {
    category: 'Causal inference & experimental designs',
    modules: [
      { key: 'rct', label: 'Randomized controlled trials (RCTs)', desc: 'Gold standard for causal claims.' },
      { key: 'factorial_fractional', label: 'Factorial & fractional factorial designs', desc: 'Test multiple factors / interactions.' },
      { key: 'quasi_experimental', label: 'Quasi-experimental designs', desc: 'DiD, RDD, synthetic controls.' },
      { key: 'instrumental_variables', label: 'Instrumental variables (IV)', desc: 'Handle endogeneity.' },
      { key: 'mediation_moderation', label: 'Mediation & moderation analysis', desc: 'Probe indirect and conditional effects.' },
    ],
  },
  {
    category: 'Advanced & specialized',
    modules: [
      { key: 'latent_growth_curve', label: 'Latent growth curve modeling', desc: 'Model trajectories over time.' },
      { key: 'dynamic_structural', label: 'Dynamic structural models', desc: 'Economics/behavioral dynamics.' },
      { key: 'multistate_markov', label: 'Multistate models / Markov models', desc: 'Systems with transitions (e.g., health states).' },
      { key: 'tda', label: 'Topological data analysis (TDA)', desc: 'Study shape of data (advanced).' },
      { key: 'sparse_regularization', label: 'Sparse modeling & regularization', desc: 'LASSO, Ridge for high-dimensional predictors.' },
      { key: 'causal_discovery', label: 'Causal discovery algorithms', desc: 'Learn causal graphs from data (e.g., PC algorithm).' },
    ],
  },
  {
    category: 'Reporting & quality assurance',
    modules: [
      { key: 'prisma', label: 'PRISMA for systematic reviews', desc: 'Reporting standard for reviews.' },
      { key: 'consort', label: 'CONSORT for clinical trials', desc: 'Reporting checklist for RCTs.' },
      { key: 'strobe_coreq_tripod', label: 'STROBE, COREQ, TRIPOD', desc: 'Reporting standards for observational/qualitative/prediction studies.' },
      { key: 'risk_of_bias', label: 'Risk of bias assessment', desc: 'Evaluate study-level bias (Cochrane tools).' },
      { key: 'cost_effectiveness', label: 'Cost-effectiveness / health economics', desc: 'QALYs, ICERs.' },
      { key: 'policy_cost_benefit', label: 'Policy impact & cost–benefit analysis', desc: 'Estimate social/economic impacts.' },
    ],
  },
];

const StatisticalResearchOrchestratorPage: React.FC = () => {
  const [step, setStep] = useState(1);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [loadingProgress, setLoadingProgress] = useState(0);
  const [loadingMessage, setLoadingMessage] = useState('Initializing analysis...');
  const [scrollY, setScrollY] = useState(0);
  const heroRef = useRef<HTMLDivElement>(null);
  const formRef = useRef<HTMLDivElement>(null);

  // Form state
  const [jobId, setJobId] = useState(`job-${Date.now()}`);
  const [title, setTitle] = useState('');
  const [objectives, setObjectives] = useState<string[]>(['']);
  const [hypotheses, setHypotheses] = useState<string[]>(['']);
  const [researchQuestions, setResearchQuestions] = useState<string[]>(['']);
  const [design, setDesign] = useState('');
  const [sampleSize, setSampleSize] = useState<number | ''>('');
  const [samplingMethod, setSamplingMethod] = useState('');
  const [variables, setVariables] = useState<Variable[]>([
    { name: '', type: 'nominal', role: 'iv' }
  ]);
  const [methodologyNotes, setMethodologyNotes] = useState('');
  const [nRows, setNRows] = useState<number | ''>('');
  const [nColumns, setNColumns] = useState<number | ''>('');
  const [missingValues, setMissingValues] = useState<Record<string, number>>({});
  const [sampleRows, setSampleRows] = useState<Record<string, any>[]>([]);
  const [analysisOutputs, setAnalysisOutputs] = useState<AnalysisOutput[]>([]);
  const [tasks, setTasks] = useState<string[]>(['recommend_tests', 'explain_what_why_how', 'interpret_results', 'generate_html_report']);
  const [maxTokens, setMaxTokens] = useState(4000);
  const [autoSelectTests, setAutoSelectTests] = useState(true);
  const [selectedModules, setSelectedModules] = useState<string[]>([
    'data_preparation',
    'descriptive_stats',
    'association',
    'parametric_tests',
    'non_parametric_tests',
    'regression',
    'reliability',
    'validity',
    'assumption_tests'
  ]);
  const [chatInput, setChatInput] = useState('');
  const [chatMessages, setChatMessages] = useState<{ role: 'user' | 'assistant'; content: string }[]>([]);
  const [resultView, setResultView] = useState<'executive' | 'results' | 'chat'>('executive');
  const [questionnaireFile, setQuestionnaireFile] = useState<File | null>(null);
  const [questionnaireText, setQuestionnaireText] = useState('');
  const [questionnaireParsing, setQuestionnaireParsing] = useState(false);
  const [questionnaireError, setQuestionnaireError] = useState<string | null>(null);
  const [questionnaireMetadata, setQuestionnaireMetadata] = useState<{ file_name: string; file_type: string; word_count: number } | null>(null);
  const [isQuestionnaireDragging, setIsQuestionnaireDragging] = useState(false);

  // AI intake assistant state (what to upload / provide before analysis)
  const [intakeSuggestions, setIntakeSuggestions] = useState<string>('');
  const [intakeLoading, setIntakeLoading] = useState(false);
  const [intakeError, setIntakeError] = useState<string | null>(null);
  const [intakeStage, setIntakeStage] = useState<'basic' | 'inputs'>('basic');

  // Formal international research report styling + auto-charts from tables
  const enhancedExecutiveReport = useMemo(() => {
    const raw = result?.executive_summary_report || result?.html_report;
    return raw ? enhanceFormalReportHTML(raw, title || 'Executive Summary') : '';
  }, [result?.executive_summary_report, result?.html_report, title]);
  const enhancedResultsReport = useMemo(() => {
    const raw = result?.results_chapter_report || result?.html_report;
    return raw ? enhanceFormalReportHTML(raw, title || 'Results Chapter') : '';
  }, [result?.results_chapter_report, result?.html_report, title]);
  const enhancedFullReport = useMemo(() => {
    const raw = result?.html_report;
    return raw ? enhanceFormalReportHTML(raw, title || 'Research Report') : '';
  }, [result?.html_report, title]);

  // Calculate descriptive statistics from sample data
  const calculateDescriptiveStats = (data: Record<string, any>[]) => {
    if (data.length === 0) return null;
    
    const stats: Record<string, any> = {};
    const columns = Object.keys(data[0]);
    
    columns.forEach(col => {
      const values = data.map(row => {
        const val = row[col];
        if (val === '' || val === null || val === undefined) return null;
        const num = Number(val);
        return isNaN(num) ? null : num;
      }).filter(v => v !== null) as number[];
      
      if (values.length > 0) {
        const sorted = [...values].sort((a, b) => a - b);
        const sum = values.reduce((a, b) => a + b, 0);
        const mean = sum / values.length;
        const variance = values.reduce((acc, val) => acc + Math.pow(val - mean, 2), 0) / values.length;
        const stdDev = Math.sqrt(variance);
        
        stats[col] = {
          mean: mean.toFixed(3),
          median: (sorted.length % 2 === 0 
            ? (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2 
            : sorted[Math.floor(sorted.length / 2)]).toFixed(3),
          stdDev: stdDev.toFixed(3),
          min: sorted[0].toFixed(3),
          max: sorted[sorted.length - 1].toFixed(3),
          count: values.length,
          isNumeric: true
        };
      } else {
        stats[col] = { isNumeric: false, count: data.length };
      }
    });
    
    return stats;
  };

  // File upload state
  const [uploadedFile, setUploadedFile] = useState<File | null>(null);
  const [parsingFile, setParsingFile] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);
  const [parsedTables, setParsedTables] = useState<ParsedTable[]>([]);
  const [fileMetadata, setFileMetadata] = useState<any>(null);
  const [isDragging, setIsDragging] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setScrollY(window.scrollY);
    };
    window.addEventListener('scroll', handleScroll, { passive: true });
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  useEffect(() => {
    if (step === 2 && formRef.current) {
      formRef.current.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  }, [step]);

  const addObjective = () => {
    setObjectives([...objectives, '']);
  };

  const updateObjective = (index: number, value: string) => {
    const newObjectives = [...objectives];
    newObjectives[index] = value;
    setObjectives(newObjectives);
  };

  const removeObjective = (index: number) => {
    setObjectives(objectives.filter((_, i) => i !== index));
  };

  const addHypothesis = () => {
    setHypotheses([...hypotheses, '']);
  };

  const updateHypothesis = (index: number, value: string) => {
    const newHypotheses = [...hypotheses];
    newHypotheses[index] = value;
    setHypotheses(newHypotheses);
  };

  const removeHypothesis = (index: number) => {
    setHypotheses(hypotheses.filter((_, i) => i !== index));
  };

  const addVariable = () => {
    setVariables([...variables, { name: '', type: 'nominal', role: 'iv' }]);
  };

  const updateVariable = (index: number, field: keyof Variable, value: any) => {
    const newVariables = [...variables];
    newVariables[index] = { ...newVariables[index], [field]: value };
    setVariables(newVariables);
  };

  const removeVariable = (index: number) => {
    setVariables(variables.filter((_, i) => i !== index));
  };

  const toggleTask = (task: string) => {
    if (tasks.includes(task)) {
      setTasks(tasks.filter(t => t !== task));
    } else {
      setTasks([...tasks, task]);
    }
  };

  const toggleModule = (moduleKey: string) => {
    if (selectedModules.includes(moduleKey)) {
      setSelectedModules(selectedModules.filter(m => m !== moduleKey));
    } else {
      setSelectedModules([...selectedModules, moduleKey]);
    }
  };

  const handleChatSubmit = async () => {
    if (!chatInput.trim() || !result) return;
    const newMessage = { role: 'user' as const, content: chatInput.trim() };
    const nextMessages = [...chatMessages, newMessage];
    setChatMessages(nextMessages);
    setChatInput('');

    const reportContext = {
      title: result.title,
      summary_simple: result.summary_simple,
      test_results: result.test_results,
      analysis_outputs: result.analysis_outputs,
      questionnaire_analysis_outputs: result.questionnaire_analysis_outputs,
    };

    const response = await apiFetch('/api/ai/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: nextMessages,
        report_context: JSON.stringify(reportContext),
      }),
    });

    if (!response.ok) {
      setChatMessages([...nextMessages, { role: 'assistant', content: 'Sorry, I could not process your request. Please try again.' }]);
      return;
    }

    const data = await response.json();
    if (data?.response) {
      setChatMessages([...nextMessages, { role: 'assistant', content: data.response }]);
    } else {
      setChatMessages([...nextMessages, { role: 'assistant', content: 'No response from assistant.' }]);
    }
  };

  const handleIntakeAssist = async () => {
    if (!title.trim() && objectives.every(o => !o.trim())) {
      setIntakeError('Add at least a research title or one objective first.');
      return;
    }
    setIntakeError(null);
    setIntakeLoading(true);

    try {
      const intakeContext = {
        title: title.trim(),
        objectives: objectives.filter(o => o.trim()),
        research_questions: researchQuestions.filter(q => q.trim()),
        design: design.trim(),
        sample_size: sampleSize || nRows || '',
        sampling_method: samplingMethod.trim(),
        variables: variables.filter(v => v.name.trim()),
        methodology_notes: methodologyNotes.trim(),
      };

      const messages = [
        {
          role: 'user' as const,
          content:
            'You are Gaply DataMaestro intake assistant. Based on this study description (JSON below), tell me exactly what files, data, and details I should upload or type so that you can run a complete, correct statistical analysis. ' +
            'Group your answer into clear sections like "Core dataset", "Questionnaire & scales", "Sampling & design details", "Variables & coding", and "Other helpful files". ' +
            'For each item, explain in one short sentence WHY it is needed and HOW it will be used in the analysis. Use simple language.\n\n' +
            'Study description JSON:\n' +
            JSON.stringify(intakeContext, null, 2),
        },
      ];

      const response = await apiFetch('/api/ai/chat', {
        method: 'POST',
        body: JSON.stringify({
          mode: 'statistical_intake',
          messages,
          intake_context: JSON.stringify(intakeContext),
          max_tokens: 900,
        }),
      });

      if (!response.ok) {
        const errText = await response.text().catch(() => '');
        setIntakeError(
          errText && errText.length < 400
            ? errText
            : 'Could not get suggestions right now. Please try again.',
        );
        return;
      }

      const data = await response.json();
      if (data?.response) {
        setIntakeSuggestions(data.response);
        // Keep user on the same section; they can click "Continue" to open full inputs
      } else {
        setIntakeError('No suggestions returned. Please try again.');
      }
    } catch (e: any) {
      setIntakeError(e?.message || 'Failed to get suggestions.');
    } finally {
      setIntakeLoading(false);
    }
  };

  const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    await processFile(file);
  };

  const handleFileDrop = async (file: File) => {
    await processFile(file);
  };

  const processFile = async (file: File) => {
    setParsingFile(true);
    setParseError(null);
    setUploadedFile(file);
    setParsedTables([]);
    setFileMetadata(null);

    try {
      const fileExtension = file.name.split('.').pop()?.toLowerCase();
      const fileType = fileExtension === 'xls' || fileExtension === 'xlsx' ? 'xlsx' : 
                      fileExtension === 'tsv' ? 'tsv' : 
                      fileExtension || 'txt';

      if (fileExtension === 'csv' || fileExtension === 'tsv' || fileExtension === 'txt') {
        await parseCSVFile(file, fileExtension === 'tsv');
      } else if (fileExtension === 'xls' || fileExtension === 'xlsx') {
        await parseExcelFile(file);
      } else if (fileExtension === 'json') {
        await parseJSONFile(file);
      } else if (fileExtension === 'pdf' || fileExtension === 'docx') {
        // For PDF/DOCX, we'll need backend processing
        await uploadToBackendForParsing(file);
      } else {
        throw new Error(`Unsupported file format (.${fileExtension}). Please upload CSV, XLS, XLSX, TSV, TXT, JSON, PDF, or DOCX.`);
      }
    } catch (err: any) {
      setParseError(err?.message || 'Failed to parse file. Please check the file format.');
      setUploadedFile(null);
    } finally {
      setParsingFile(false);
    }
  };

  const parseCSVFile = async (file: File, isTSV: boolean = false) => {
    const text = await file.text();
    const delimiter = isTSV ? '\t' : ',';
    const lines = text.split('\n').filter(line => line.trim());
    
    if (lines.length === 0) {
      throw new Error('File is empty');
    }

    const headers = lines[0].split(delimiter).map(h => h.trim().replace(/^"|"$/g, ''));
    const rows: Record<string, any>[] = [];
    const missingCounts: Record<string, number> = {};
    
    headers.forEach(header => {
      missingCounts[header] = 0;
    });

    for (let i = 1; i < lines.length; i++) {
      const values = lines[i].split(delimiter).map(v => v.trim().replace(/^"|"$/g, ''));
      const row: Record<string, any> = {};
      
      headers.forEach((header, idx) => {
        const value = values[idx] || '';
        row[header] = value;
        if (!value || value === '' || value === 'NA' || value === 'null') {
          missingCounts[header]++;
        }
      });
      
      rows.push(row);
    }

    // Detect column types
    const columns = headers.map(header => {
      const sampleValues = rows.slice(0, Math.min(10, rows.length)).map(r => r[header]).filter(v => v);
      let detectedType = 'text';
      if (sampleValues.length > 0) {
        const firstVal = sampleValues[0];
        if (!isNaN(Number(firstVal)) && firstVal !== '') {
          detectedType = 'numeric';
        } else if (!isNaN(Date.parse(firstVal))) {
          detectedType = 'date';
        }
      }
      return { name: header, detected_type: detectedType, notes: null };
    });

    const parsedTable: ParsedTable = {
      table_id: `table-${Date.now()}`,
      source_file: file.name,
      sheet_name: null,
      n_rows: rows.length,
      n_columns: headers.length,
      columns,
      sample_rows: rows,
      parsing_warnings: [],
    };

    setParsedTables([parsedTable]);
    setFileMetadata({
      file_name: file.name,
      file_type: isTSV ? 'tsv' : 'csv',
      file_size_bytes: file.size,
      parsing_status: 'ok',
      ocr_performed: false,
      parsing_warnings: [],
    });

    setNRows(rows.length);
    setNColumns(headers.length);
    setSampleRows(rows);
    setMissingValues(missingCounts);
  };

  const parseExcelFile = async (file: File) => {
    const arrayBuffer = await readFileAsArrayBuffer(file);
    const workbook = XLSX.read(arrayBuffer, { type: 'array' });
    
    const tables: ParsedTable[] = [];
    
    workbook.SheetNames.forEach((sheetName, sheetIdx) => {
      const worksheet = workbook.Sheets[sheetName];
      const jsonData = XLSX.utils.sheet_to_json(worksheet, { header: 1 });
      
      if (jsonData.length === 0) return;
      
      const headers = (jsonData[0] as any[]).map((h: any) => String(h || '').trim()).filter(h => h);
      if (headers.length === 0) return;
      
      const rows: Record<string, any>[] = [];
      const missingCounts: Record<string, number> = {};
      
      headers.forEach(header => {
        missingCounts[header] = 0;
      });
      
      for (let i = 1; i < jsonData.length; i++) {
        const rowData = jsonData[i] as any[];
        const row: Record<string, any> = {};
        
        headers.forEach((header, idx) => {
          const value = rowData[idx] !== undefined ? String(rowData[idx] || '').trim() : '';
          row[header] = value;
          if (!value || value === '' || value === 'NA' || value === 'null') {
            missingCounts[header]++;
          }
        });
        
        rows.push(row);
      }

      const columns = headers.map(header => {
        const sampleValues = rows.slice(0, Math.min(10, rows.length)).map(r => r[header]).filter(v => v);
        let detectedType = 'text';
        if (sampleValues.length > 0) {
          const firstVal = sampleValues[0];
          if (!isNaN(Number(firstVal)) && firstVal !== '') {
            detectedType = 'numeric';
          } else if (!isNaN(Date.parse(firstVal))) {
            detectedType = 'date';
          }
        }
        return { name: header, detected_type: detectedType, notes: null };
      });

      tables.push({
        table_id: `table-${Date.now()}-${sheetIdx}`,
        source_file: file.name,
        sheet_name: sheetName,
        n_rows: rows.length,
        n_columns: headers.length,
        columns,
        sample_rows: rows,
        parsing_warnings: [],
      });
    });

    setParsedTables(tables);
    setFileMetadata({
      file_name: file.name,
      file_type: file.name.endsWith('.xlsx') ? 'xlsx' : 'xls',
      file_size_bytes: file.size,
      parsing_status: 'ok',
      ocr_performed: false,
      parsing_warnings: [],
    });

    // Use largest table for dataset summary
    if (tables.length > 0) {
      const primaryTable = tables.reduce((max, t) => (t.n_rows > max.n_rows ? t : max), tables[0]);
      setNRows(primaryTable.n_rows);
      setNColumns(primaryTable.n_columns);
      setSampleRows(primaryTable.sample_rows);
    }
  };

  const parseJSONFile = async (file: File) => {
    const text = await file.text();
    const data = JSON.parse(text);
    
    if (Array.isArray(data)) {
      if (data.length === 0) {
        throw new Error('JSON array is empty');
      }
      
      const headers = Object.keys(data[0]);
      const rows = data.map((item: any) => {
        const row: Record<string, any> = {};
        headers.forEach(header => {
          row[header] = item[header] ?? '';
        });
        return row;
      });

      const missingCounts: Record<string, number> = {};
      headers.forEach(header => {
        missingCounts[header] = data.filter((item: any) => 
          !item[header] || item[header] === '' || item[header] === null
        ).length;
      });

      const columns = headers.map(header => {
        const sampleValues = rows.slice(0, Math.min(10, rows.length)).map(r => r[header]).filter(v => v);
        let detectedType = 'text';
        if (sampleValues.length > 0) {
          const firstVal = sampleValues[0];
          if (!isNaN(Number(firstVal)) && firstVal !== '') {
            detectedType = 'numeric';
          } else if (!isNaN(Date.parse(firstVal))) {
            detectedType = 'date';
          }
        }
        return { name: header, detected_type: detectedType, notes: null };
      });

      const parsedTable: ParsedTable = {
        table_id: `table-${Date.now()}`,
        source_file: file.name,
        sheet_name: null,
        n_rows: rows.length,
        n_columns: headers.length,
        columns,
        sample_rows: rows,
        parsing_warnings: [],
      };

      setParsedTables([parsedTable]);
      setFileMetadata({
        file_name: file.name,
        file_type: 'json',
        file_size_bytes: file.size,
        parsing_status: 'ok',
        ocr_performed: false,
        parsing_warnings: [],
      });

      setNRows(rows.length);
      setNColumns(headers.length);
      setSampleRows(rows);
      setMissingValues(missingCounts);
    } else {
      throw new Error('JSON file must contain an array of objects');
    }
  };

  const uploadToBackendForParsing = async (file: File) => {
    // For PDF/DOCX, send to backend for parsing
    const formData = new FormData();
    formData.append('file', file);

    try {
      const response = await apiFetch('/api/ai/parse-dataset', {
        method: 'POST',
        body: formData,
      });

      if (!response.ok) {
        throw new Error('Backend parsing failed. Please convert PDF/DOCX to CSV or use manual entry.');
      }

      const data = await response.json();
      
      if (data.parsed_tables) {
        setParsedTables(data.parsed_tables);
      }
      if (data.file_metadata) {
        setFileMetadata(data.file_metadata);
      }
      if (data.dataset_summary) {
        setNRows(data.dataset_summary.n_rows || 0);
        setNColumns(data.dataset_summary.n_columns || 0);
        setSampleRows(data.dataset_summary.sample_rows || []);
        setMissingValues(data.dataset_summary.missing_values_summary || {});
      }
    } catch (err: any) {
      throw new Error('PDF/DOCX parsing requires backend processing. Please convert to CSV format or use manual entry.');
    }
  };

  const uploadQuestionnaireToBackend = async (file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    setQuestionnaireParsing(true);
    setQuestionnaireError(null);

    try {
      const response = await apiFetch('/api/ai/parse-questionnaire', {
        method: 'POST',
        body: formData,
      });

      if (!response.ok) {
        throw new Error('Failed to parse questionnaire. Please upload a valid PDF, DOCX, or TXT file.');
      }

      const data = await response.json();
      if (data?.questionnaire_text) {
        setQuestionnaireText(data.questionnaire_text);
      }
      if (data?.file_metadata) {
        setQuestionnaireMetadata(data.file_metadata);
      }
    } catch (err: any) {
      setQuestionnaireError(err?.message || 'Failed to parse questionnaire file.');
      setQuestionnaireText('');
      setQuestionnaireMetadata(null);
    } finally {
      setQuestionnaireParsing(false);
    }
  };

  const readFileAsArrayBuffer = (file: File): Promise<ArrayBuffer> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = (e) => resolve(e.target?.result as ArrayBuffer);
      reader.onerror = reject;
      reader.readAsArrayBuffer(file);
    });
  };

  const handleSubmit = async () => {
    if (!title.trim()) {
      setError('Please enter a research title');
      return;
    }

    setLoading(true);
    setError(null);
    setLoadingProgress(0);
    setLoadingMessage('Preparing your analysis...');
    setStep(3);

    // Simulate progress for better UX (ChatGPT-like smooth experience)
    let progressInterval: NodeJS.Timeout;
    let messageInterval: NodeJS.Timeout;
    
    progressInterval = setInterval(() => {
      setLoadingProgress(prev => {
        if (prev >= 90) return prev;
        return prev + Math.random() * 5;
      });
    }, 500);

    messageInterval = setInterval(() => {
      const messages = [
        'Processing your data...',
        'Running statistical checks...',
        'Building tables and figures...',
        'Finalizing the report...'
      ];
      setLoadingMessage(messages[Math.floor(Math.random() * messages.length)]);
    }, 2000);

    try {
      // Use full dataset rows when available
      let finalNRows = nRows;
      let finalNColumns = nColumns;
      let finalSampleRows = sampleRows;
      
      if (parsedTables.length > 0 && (!finalNRows || finalNRows === 0)) {
        finalNRows = parsedTables[0].n_rows;
        finalNColumns = parsedTables[0].n_columns;
        finalSampleRows = parsedTables[0].sample_rows;
      }
      
      // Ensure we have minimum required data - but allow proceeding with parsed table data
      if ((!finalNRows || finalNRows === 0) && parsedTables.length === 0) {
        setError('Please provide dataset information (number of rows) or upload a dataset file');
        setLoading(false);
        setStep(2);
        return;
      }

      const payload: any = {
        job_id: jobId,
        title: title.trim(),
        objectives: objectives.filter(o => o.trim()).length > 0 ? objectives.filter(o => o.trim()) : ['Analyze the provided dataset'],
        hypotheses: hypotheses.filter(h => h.trim()).length > 0 ? hypotheses.filter(h => h.trim()) : null,
        research_questions: researchQuestions.filter(q => q.trim()).length > 0 ? researchQuestions.filter(q => q.trim()) : null,
        methodology: {
          design: design.trim() || 'Observational study',
          sample_size: sampleSize || nRows || 0,
          sampling_method: samplingMethod.trim() || 'Not specified',
          variable_list: variables.filter(v => v.name.trim()).length > 0 ? variables.filter(v => v.name.trim()) : [],
          notes: methodologyNotes.trim() || 'Analysis requested via Statistical Research Orchestrator',
        },
        dataset_summary: {
          n_rows: finalNRows || 0,
          n_columns: finalNColumns || 0,
          missing_values_summary: missingValues || {},
          sample_rows: finalSampleRows.length > 0 ? finalSampleRows : (parsedTables.length > 0 && parsedTables[0].sample_rows ? parsedTables[0].sample_rows : []),
        },
        questionnaire_text: questionnaireText.trim() ? questionnaireText.trim() : null,
        questionnaire_file: questionnaireMetadata,
        questionnaire_evaluations: questionnaireText.trim() ? questionnaireText.trim() : null,
        analysis_outputs: analysisOutputs || [],
        tasks: tasks.length > 0 ? tasks : ['recommend_tests', 'explain_what_why_how', 'generate_html_report'],
        max_tokens_for_response: maxTokens || 8000,
        selected_modules: autoSelectTests ? [] : selectedModules,
        report_instructions: {
          style: 'formal_international_research',
          depth: 'very_deep',
          require_figures: true,
          figure_types: ['bar', 'line', 'distribution', 'scatter', 'grouped_bar'],
          table_and_figure_captions: true,
          apa_format: true,
        },
      };

      // Add parsed tables and file metadata if file was uploaded
      if (parsedTables.length > 0) {
        payload.parsed_tables = parsedTables;
      }
      if (fileMetadata) {
        payload.file_metadata = fileMetadata;
      }

      // No timeout - let it process until completion for best results
      try {
        const response = await apiFetch('/api/ai/statistical-research-orchestrator', {
          method: 'POST',
          body: JSON.stringify(payload),
        });

        if (!response.ok) {
          const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
          const msg = response.status === 503
            ? (errorData.error || 'DataMaestro analysis service is not configured. Your admin should set STATISTICAL_ORCHESTRATOR_URL on the backend and deploy the statistical orchestrator. Use OPENAI_MODEL=code-gpt-5.2-codex on that service for best results.')
            : (errorData.error || `Server error: ${response.status}`);
          throw new Error(msg);
        }

        if (progressInterval) clearInterval(progressInterval);
        if (messageInterval) clearInterval(messageInterval);
        setLoadingProgress(100);
        setLoadingMessage('Analysis complete!');

        const resultData = await response.json();
        console.log('Received result data:', resultData);
        console.log('Result status:', resultData.status);
        
        // If status is external_check_required, show helpful message but still display what we have
        if (resultData.status === 'external_check_required') {
          console.warn('Analysis requires additional data:', resultData.warnings);
        }
        
        // Smooth transition to results
        setTimeout(() => {
          setResult(resultData);
          setStep(4);
        }, 300);
      } catch (fetchErr: any) {
        throw fetchErr;
      }
    } catch (err: any) {
      if (progressInterval) clearInterval(progressInterval);
      if (messageInterval) clearInterval(messageInterval);
      setError(err?.message || 'Failed to process analysis. Please try again.');
      setStep(2);
    } finally {
      setLoading(false);
      setLoadingProgress(0);
      if (progressInterval) clearInterval(progressInterval);
      if (messageInterval) clearInterval(messageInterval);
    }
  };

  const resetForm = () => {
    setStep(1);
    setResult(null);
    setError(null);
    setJobId(`job-${Date.now()}`);
    setTitle('');
    setObjectives(['']);
    setHypotheses(['']);
    setResearchQuestions(['']);
    setDesign('');
    setSampleSize('');
    setSamplingMethod('');
    setVariables([{ name: '', type: 'nominal', role: 'iv' }]);
    setMethodologyNotes('');
    setNRows('');
    setNColumns('');
    setMissingValues({});
    setSampleRows([]);
    setAnalysisOutputs([]);
    setParsedTables([]);
    setFileMetadata(null);
    setUploadedFile(null);
  };

  return (
    <div className="statistical-research-page">
      <SEO
        title="Statistical Research Orchestrator | Gaply"
        description="Advanced statistical analysis orchestrator for research. Upload datasets, get test recommendations, interpretations, and comprehensive HTML reports."
        keywords="statistical analysis, research orchestrator, statistical tests, data analysis, research methodology"
      />

      {/* Hero Section */}
      {step === 1 && (
        <section className="sr-hero" ref={heroRef}>
          <div className="sr-hero-background" style={{ transform: `translateY(${scrollY * 0.5}px)` }}></div>
          <div className="sr-hero-content">
            <h1 className="sr-hero-title">
              Statistical Research
              <br />
              <span className="sr-hero-title-accent">Orchestrator</span>
            </h1>
            <p className="sr-hero-subtitle">
              Transform your research data into comprehensive statistical analysis reports.
              Upload datasets, get expert recommendations, interpretations, and publication-ready HTML reports.
            </p>
            <button 
              className="sr-button-primary"
              onClick={() => setStep(2)}
            >
              Get Started
              <span className="sr-button-arrow">→</span>
            </button>
          </div>
          <div className="sr-hero-visual">
            <div className="sr-visual-card">
              <div className="sr-card-icon">📊</div>
              <h3>File Upload</h3>
              <p>Support for CSV, Excel, PDF, DOCX, and more</p>
            </div>
            <div className="sr-visual-card">
              <div className="sr-card-icon">📈</div>
              <h3>Test Recommendations</h3>
              <p>AI-powered statistical test suggestions</p>
            </div>
            <div className="sr-visual-card">
              <div className="sr-card-icon">📄</div>
              <h3>HTML Reports</h3>
              <p>Publication-ready analysis reports</p>
            </div>
          </div>
        </section>
      )}

      {/* Form Section */}
      {step === 2 && (
        <section className="sr-form-section" ref={formRef}>
          <div className="sr-form-container">
            <div className="sr-form-header">
              <button className="sr-back-button" onClick={() => setStep(1)}>
                ← Back
              </button>
              <h2>Research Information</h2>
              <p>Provide details about your research study</p>
            </div>

            <div className="sr-form-content">
              {/* Basic Information */}
              <div className="sr-form-group">
                <label className="sr-label">Research Title *</label>
                <input
                  type="text"
                  className="sr-input"
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  placeholder="Enter your research title"
                />
              </div>

              {/* Objectives */}
              <div className="sr-form-group">
                <label className="sr-label">Research Objectives</label>
                {objectives.map((obj, idx) => (
                  <div key={idx} className="sr-input-group">
                    <input
                      type="text"
                      className="sr-input"
                      value={obj}
                      onChange={(e) => updateObjective(idx, e.target.value)}
                      placeholder={`Objective ${idx + 1}`}
                    />
                    {objectives.length > 1 && (
                      <button
                        className="sr-remove-button"
                        onClick={() => removeObjective(idx)}
                      >
                        ×
                      </button>
                    )}
                  </div>
                ))}
                <button className="sr-add-button" onClick={addObjective}>
                  + Add Objective
                </button>
              </div>

              {/* Hypotheses */}
              {intakeStage === 'inputs' && (
                <>
                  {/* Hypotheses */}
                  <div className="sr-form-group">
                    <label className="sr-label">Hypotheses (Optional)</label>
                    {hypotheses.map((hyp, idx) => (
                      <div key={idx} className="sr-input-group">
                        <input
                          type="text"
                          className="sr-input"
                          value={hyp}
                          onChange={(e) => updateHypothesis(idx, e.target.value)}
                          placeholder={`Hypothesis ${idx + 1}`}
                        />
                        {hypotheses.length > 1 && (
                          <button
                            className="sr-remove-button"
                            onClick={() => removeHypothesis(idx)}
                          >
                            ×
                          </button>
                        )}
                      </div>
                    ))}
                    <button className="sr-add-button" onClick={addHypothesis}>
                      + Add Hypothesis
                    </button>
                  </div>

                  {/* Methodology */}
                  <div className="sr-form-group">
                    <label className="sr-label">Research Design</label>
                    <input
                      type="text"
                      className="sr-input"
                      value={design}
                      onChange={(e) => setDesign(e.target.value)}
                      placeholder="e.g., Experimental, Observational, Cross-sectional"
                    />
                  </div>

                  <div className="sr-form-row">
                    <div className="sr-form-group">
                      <label className="sr-label">Sample Size</label>
                      <input
                        type="number"
                        className="sr-input"
                        value={sampleSize}
                        onChange={(e) => setSampleSize(e.target.value ? parseInt(e.target.value) : '')}
                        placeholder="e.g., 100"
                      />
                    </div>
                    <div className="sr-form-group">
                      <label className="sr-label">Sampling Method</label>
                      <input
                        type="text"
                        className="sr-input"
                        value={samplingMethod}
                        onChange={(e) => setSamplingMethod(e.target.value)}
                        placeholder="e.g., Random sampling, Convenience sampling"
                      />
                    </div>
                  </div>

                  {/* Variables */}
                  <div className="sr-form-group">
                    <label className="sr-label">Variables</label>
                    {variables.map((var_, idx) => (
                      <div key={idx} className="sr-variable-row">
                        <input
                          type="text"
                          className="sr-input"
                          value={var_.name}
                          onChange={(e) => updateVariable(idx, 'name', e.target.value)}
                          placeholder="Variable name"
                        />
                        <select
                          className="sr-select"
                          value={var_.type}
                          onChange={(e) => updateVariable(idx, 'type', e.target.value as Variable['type'])}
                        >
                          <option value="nominal">Nominal</option>
                          <option value="ordinal">Ordinal</option>
                          <option value="interval">Interval</option>
                          <option value="ratio">Ratio</option>
                        </select>
                        <select
                          className="sr-select"
                          value={var_.role}
                          onChange={(e) => updateVariable(idx, 'role', e.target.value as Variable['role'])}
                        >
                          <option value="iv">Independent Variable</option>
                          <option value="dv">Dependent Variable</option>
                          <option value="covariate">Covariate</option>
                          <option value="id">ID</option>
                        </select>
                        {variables.length > 1 && (
                          <button
                            className="sr-remove-button"
                            onClick={() => removeVariable(idx)}
                          >
                            ×
                          </button>
                        )}
                      </div>
                    ))}
                    <button className="sr-add-button" onClick={addVariable}>
                      + Add Variable
                    </button>
                  </div>
                </>
              )}

              {/* File Upload Section */}
              <div className="sr-form-group">
                <h3 className="sr-section-title">Upload Dataset File</h3>
                <div 
                  className={`sr-file-dropzone ${isDragging ? 'sr-dragging' : ''}`}
                  onDragOver={(e) => {
                    e.preventDefault();
                    setIsDragging(true);
                  }}
                  onDragLeave={(e) => {
                    e.preventDefault();
                    setIsDragging(false);
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    setIsDragging(false);
                    const file = e.dataTransfer.files[0];
                    if (file) {
                      handleFileDrop(file);
                    }
                  }}
                >
                  <input
                    type="file"
                    id="dataset-upload"
                    className="sr-file-input"
                    accept=".csv,.xls,.xlsx,.tsv,.txt,.json,.pdf,.docx"
                    multiple={false}
                    onChange={handleFileUpload}
                  />
                  <label htmlFor="dataset-upload" className="sr-file-dropzone-label">
                    <div className="sr-file-dropzone-icon">📁</div>
                    <div className="sr-file-dropzone-text">
                      <strong>Drag & drop</strong> your dataset file here
                      <br />
                      <span>or click to browse</span>
                    </div>
                    <div className="sr-file-dropzone-formats">
                      Supported: CSV, XLS, XLSX, TSV, TXT, JSON, PDF, DOCX
                    </div>
                  </label>
                </div>
                
                {uploadedFile && (
                  <div className="sr-uploaded-file">
                    <span className="sr-file-name">{uploadedFile.name}</span>
                    <span className="sr-file-size">{(uploadedFile.size / 1024).toFixed(2)} KB</span>
                    <button
                      className="sr-file-remove"
                      onClick={() => {
                        setUploadedFile(null);
                        setParsedTables([]);
                        setFileMetadata(null);
                        setNRows('');
                        setNColumns('');
                        setSampleRows([]);
                        setMissingValues({});
                      }}
                    >
                      ×
                    </button>
                  </div>
                )}

                {parsingFile && (
                  <div className="sr-parsing-status">
                    <div className="sr-loading-spinner-small"></div>
                    <span>Parsing dataset...</span>
                  </div>
                )}

                {parseError && (
                  <div className="sr-error-message">
                    {parseError}
                  </div>
                )}

                {/* Parsed Tables Preview */}
                {parsedTables.length > 0 && (
                  <div className="sr-parsed-tables-preview">
                    <h4>Parsed Tables ({parsedTables.length})</h4>
                    {parsedTables.map((table, idx) => (
                      <div key={idx} className="sr-parsed-table-card">
                        <div className="sr-table-header">
                          <strong>{table.source_file}</strong>
                          {table.sheet_name && <span className="sr-sheet-name">Sheet: {table.sheet_name}</span>}
                          <span className="sr-table-size">{table.n_rows} rows × {table.n_columns} cols</span>
                        </div>
                        {table.columns.length > 0 && (
                          <div className="sr-table-columns">
                            <strong>Columns:</strong> {table.columns.map(c => c.name).join(', ')}
                          </div>
                        )}
                        {table.parsing_warnings.length > 0 && (
                          <div className="sr-parsing-warnings">
                            <strong>Warnings:</strong> {table.parsing_warnings.join('; ')}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* Questionnaire Upload Section */}
              <div className="sr-form-group">
                <h3 className="sr-section-title">Upload Questionnaire (PDF, DOCX, TXT)</h3>
                <div
                  className={`sr-file-dropzone ${isQuestionnaireDragging ? 'sr-dragging' : ''}`}
                  onDragOver={(e) => {
                    e.preventDefault();
                    setIsQuestionnaireDragging(true);
                  }}
                  onDragLeave={(e) => {
                    e.preventDefault();
                    setIsQuestionnaireDragging(false);
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    setIsQuestionnaireDragging(false);
                    const file = e.dataTransfer.files[0];
                    if (file) {
                      setQuestionnaireFile(file);
                      uploadQuestionnaireToBackend(file);
                    }
                  }}
                >
                  <input
                    type="file"
                    id="questionnaire-upload"
                    className="sr-file-input"
                    accept=".pdf,.docx,.txt"
                    multiple={false}
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (file) {
                        setQuestionnaireFile(file);
                        uploadQuestionnaireToBackend(file);
                      }
                    }}
                  />
                  <label htmlFor="questionnaire-upload" className="sr-file-dropzone-label">
                    <div className="sr-file-dropzone-icon">🧾</div>
                    <div className="sr-file-dropzone-text">
                      <strong>Drag & drop</strong> your questionnaire here
                      <br />
                      <span>or click to browse</span>
                    </div>
                    <div className="sr-file-dropzone-formats">
                      Supported: PDF, DOCX, TXT
                    </div>
                  </label>
                </div>

                {questionnaireFile && (
                  <div className="sr-uploaded-file">
                    <span className="sr-file-name">{questionnaireFile.name}</span>
                    <span className="sr-file-size">{(questionnaireFile.size / 1024).toFixed(2)} KB</span>
                    <button
                      className="sr-file-remove"
                      onClick={() => {
                        setQuestionnaireFile(null);
                        setQuestionnaireText('');
                        setQuestionnaireMetadata(null);
                        setQuestionnaireError(null);
                      }}
                    >
                      ×
                    </button>
                  </div>
                )}

                {questionnaireParsing && (
                  <div className="sr-parsing-status">
                    <div className="sr-loading-spinner-small"></div>
                    <span>Parsing questionnaire...</span>
                  </div>
                )}

                {questionnaireError && (
                  <div className="sr-error-message">
                    {questionnaireError}
                  </div>
                )}

                {questionnaireText && (
                  <div className="sr-parsed-tables-preview">
                    <h4>Questionnaire Extract (Preview)</h4>
                    <div className="sr-report-context">
                      {questionnaireText.slice(0, 1200)}
                      {questionnaireText.length > 1200 ? '…' : ''}
                    </div>
                  </div>
                )}
              </div>

              {/* AI Intake Assistant: what to upload / provide */}
              <div className="sr-form-group">
                <h3 className="sr-section-title">Not sure what to upload?</h3>
                <p className="sr-intake-description">
                  Gaply can read your research title, objectives, and methodology and suggest an exact checklist
                  of datasets, questionnaires, and details to provide for the best analysis.
                </p>
                <button
                  type="button"
                  className="sr-button-secondary"
                  onClick={handleIntakeAssist}
                  disabled={intakeLoading}
                >
                  {intakeLoading ? 'Thinking…' : 'Ask Gaply what to upload'}
                </button>
                {intakeError && (
                  <div className="sr-error-message" style={{ marginTop: 10 }}>
                    {intakeError}
                  </div>
                )}
                {intakeSuggestions && (
                  <div className="sr-intake-panel">
                    <div
                      className="sr-intake-content"
                      // Model may return markdown or plain text; render as simple HTML
                      dangerouslySetInnerHTML={{ __html: intakeSuggestions }}
                    />
                  </div>
                )}
                {intakeStage === 'basic' && (
                  <div className="sr-form-row" style={{ marginTop: 16, gap: 12, flexWrap: 'wrap' }}>
                    <button
                      type="button"
                      className="sr-button-primary"
                      onClick={() => setIntakeStage('inputs')}
                    >
                      Continue to upload & run analysis
                    </button>
                    <button
                      type="button"
                      className="sr-button-secondary"
                      onClick={() => setIntakeStage('inputs')}
                    >
                      Configure manually
                    </button>
                  </div>
                )}
              </div>

              {/* Dataset Summary, sample preview, and test modules only after intake stage */}
              {intakeStage === 'inputs' && (
                <>
                  {/* Dataset Summary */}
                  <div className="sr-form-group">
                    <h3 className="sr-section-title">Dataset Summary</h3>
                    <div className="sr-form-row">
                      <div className="sr-form-group">
                        <label className="sr-label">Number of Rows</label>
                        <input
                          type="number"
                          className="sr-input"
                          value={nRows}
                          onChange={(e) => setNRows(e.target.value ? parseInt(e.target.value) : '')}
                          placeholder="e.g., 1000"
                          disabled={parsingFile}
                        />
                      </div>
                      <div className="sr-form-group">
                        <label className="sr-label">Number of Columns</label>
                        <input
                          type="number"
                          className="sr-input"
                          value={nColumns}
                          onChange={(e) => setNColumns(e.target.value ? parseInt(e.target.value) : '')}
                          placeholder="e.g., 10"
                          disabled={parsingFile}
                        />
                      </div>
                    </div>
                  </div>

                  {/* Sample Data Preview */}
                  {sampleRows.length > 0 && (
                    <div className="sr-form-group">
                      <h4 className="sr-preview-title">Sample Data Preview (First 5 rows)</h4>
                      <div className="sr-preview-table-container">
                        <table className="sr-preview-table">
                          <thead>
                            <tr>
                              {Object.keys(sampleRows[0] || {}).map((key) => (
                                <th key={key}>{key}</th>
                              ))}
                            </tr>
                          </thead>
                          <tbody>
                            {sampleRows.slice(0, 5).map((row, idx) => (
                              <tr key={idx}>
                                {Object.values(row).map((val, vIdx) => (
                                  <td key={vIdx}>{String(val)}</td>
                                ))}
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    </div>
                  )}

                  {/* Test Modules */}
                  <div className="sr-form-group">
                    <h3 className="sr-section-title">Test Modules (Choose or Auto)</h3>
                    <label className="sr-checkbox-label">
                      <input
                        type="checkbox"
                        checked={autoSelectTests}
                        onChange={() => setAutoSelectTests(!autoSelectTests)}
                        className="sr-checkbox"
                      />
                      <span className="sr-checkbox-text">Auto-select best tests (Gaply)</span>
                    </label>
                    {!autoSelectTests && (
                      <div style={{ marginTop: '12px' }}>
                        {TEST_MODULES_BY_CATEGORY.map(({ category, modules }) => (
                          <div key={category} style={{ marginBottom: 20 }}>
                            <h4 className="sr-section-title" style={{ fontSize: '0.95rem', marginBottom: 8, color: 'var(--sr-text-secondary, #64748b)' }}>{category}</h4>
                            <div className="sr-modules-grid">
                              {modules.map((m) => (
                                <label key={m.key} className="sr-checkbox-label sr-module-item">
                                  <input
                                    type="checkbox"
                                    checked={selectedModules.includes(m.key)}
                                    onChange={() => toggleModule(m.key)}
                                    className="sr-checkbox"
                                  />
                                  <span className="sr-checkbox-text">
                                    {m.label}
                                    <span className="sr-checkbox-desc">{m.desc}</span>
                                  </span>
                                </label>
                              ))}
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
              </div>
                </>
              )}

              {error && (
                <div className="sr-error-message">
                  {error}
                </div>
              )}

              <div className="sr-form-actions">
                <button className="sr-button-secondary" onClick={() => setStep(1)}>
                  Cancel
                </button>
                <button
                  className="sr-button-primary"
                  onClick={handleSubmit}
                  disabled={loading || !title.trim()}
                >
                  {loading ? 'Processing...' : 'Analyze Research'}
                </button>
              </div>
            </div>
          </div>
        </section>
      )}

      {/* Loading Section - ChatGPT-like smooth experience */}
      {step === 3 && (
        <section className="sr-loading-section">
          <div className="sr-loading-content">
            <div className="sr-loading-spinner"></div>
            <h2>Analyzing Your Research</h2>
            <p className="sr-loading-message">{loadingMessage}</p>
            
            {/* Progress bar */}
            <div className="sr-progress-container">
              <div 
                className="sr-progress-bar" 
                style={{ width: `${Math.min(loadingProgress, 95)}%` }}
              ></div>
            </div>
            
            <div className="sr-loading-details">
              <p className="sr-loading-note">
                ⏱️ Generating comprehensive analysis...
              </p>
              <p className="sr-loading-tasks">
                {tasks.join(', ').replace(/_/g, ' ')}
              </p>
            </div>
          </div>
        </section>
      )}

      {/* Results Section */}
      {step === 4 && result && (
        <section className="sr-results-section">
          <div className="sr-results-container">
            <div className="sr-results-header">
              <button className="sr-back-button" onClick={resetForm}>
                ← New Analysis
              </button>
              <h2>Analysis Results</h2>
            </div>

            {result.status === 'error' && (
              <div className="sr-error-message">
                <h3>Error: {result.error_type}</h3>
                <p>{result.error_message}</p>
              </div>
            )}

            {(result.status === 'external_check_required' || result.status === 'partial') && (
              <div className="sr-results-content">
                {/* Show warning but still display all available results */}
                {result.warnings && result.warnings.length > 0 && (
                  <div className="sr-error-message" style={{background: 'rgba(255, 149, 0, 0.15)', borderColor: 'rgba(255, 149, 0, 0.3)', color: '#ff9500', marginBottom: '32px'}}>
                    <h3>⚠️ Note</h3>
                    <ul style={{marginTop: '16px', paddingLeft: '20px'}}>
                      {result.warnings.map((warning: string, idx: number) => (
                        <li key={idx} style={{marginBottom: '8px'}}>{warning}</li>
                      ))}
                    </ul>
                  </div>
                )}

                {/* Show ALL available results regardless of status */}
                {result.summary_simple && (
                  <div className="sr-result-card" style={{animationDelay: '0.1s'}}>
                    <h3>Summary</h3>
                    <p style={{lineHeight: '1.8', fontSize: '18px'}}>{result.summary_simple}</p>
                  </div>
                )}

                {result.recommended_tests && result.recommended_tests.length > 0 && (
                  <div className="sr-result-card" style={{animationDelay: '0.2s'}}>
                    <h3>Recommended Statistical Tests</h3>
                    <div className="sr-tests-list">
                      {result.recommended_tests.map((test: any, idx: number) => (
                        <div key={idx} className="sr-test-item">
                          <h4>{test.test_name}</h4>
                          <p>{test.why}</p>
                          <div className="sr-test-meta">
                            <span>Confidence: {(test.confidence * 100).toFixed(0)}%</span>
                            <span>Assumptions: {test.assumptions?.join(', ')}</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {result.what_why_how && result.what_why_how.length > 0 && (
                  <div className="sr-result-card" style={{animationDelay: '0.3s'}}>
                    <h3>What, Why, and How</h3>
                    {result.what_why_how.map((item: any, idx: number) => (
                      <div key={idx} className="sr-test-item" style={{marginBottom: '24px'}}>
                        <h4>{item.test_id || `Test ${idx + 1}`}</h4>
                        <p><strong>What:</strong> {item.what}</p>
                        <p><strong>Why:</strong> {item.why}</p>
                        <p><strong>How:</strong> {item.how}</p>
                        {item.assumptions_to_check && item.assumptions_to_check.length > 0 && (
                          <p><strong>Assumptions to Check:</strong> {item.assumptions_to_check.join(', ')}</p>
                        )}
                      </div>
                    ))}
                  </div>
                )}

                {result.test_results && result.test_results.length > 0 && (
                  <div className="sr-result-card" style={{animationDelay: '0.4s'}}>
                    <h3>Test Results</h3>
                    {result.test_results.map((testResult: any, idx: number) => (
                      <div key={idx} className="sr-test-result">
                        <h4>{testResult.test_name}</h4>
                        {testResult.interpretation_simple && (
                          <p className="sr-interpretation">{testResult.interpretation_simple}</p>
                        )}
                        {testResult.numeric_results && (
                          <div className="sr-numeric-results">
                            {testResult.numeric_results.statistic !== null && (
                              <div>Statistic: {testResult.numeric_results.statistic.toFixed(3)}</div>
                            )}
                            {testResult.numeric_results.p_value !== null && (
                              <div>p-value: {testResult.numeric_results.p_value < 0.001 ? '< 0.001' : testResult.numeric_results.p_value.toFixed(3)}</div>
                            )}
                            {testResult.numeric_results.effect_size !== null && (
                              <div>Effect Size: {testResult.numeric_results.effect_size.toFixed(3)}</div>
                            )}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                )}

                {result.html_report && (
                  <div className="sr-result-card" style={{animationDelay: '0.5s'}}>
                    <h3>Complete HTML Report</h3>
                    <div className="sr-html-report">
                      <iframe
                        srcDoc={enhancedFullReport}
                        title="Statistical Analysis Report"
                        className="sr-report-iframe"
                        style={{width: '100%', minHeight: '800px', border: 'none', borderRadius: '12px'}}
                      />
                    </div>
                    <button
                      className="sr-button-secondary"
                      onClick={() => {
                        const blob = new Blob([enhancedFullReport], { type: 'text/html' });
                        const url = URL.createObjectURL(blob);
                        const a = document.createElement('a');
                        a.href = url;
                        a.download = `${title.replace(/\s+/g, '_')}_report.html`;
                        a.click();
                        URL.revokeObjectURL(url);
                      }}
                    >
                      Download HTML Report
                    </button>
                  </div>
                )}

                {result.final_conclusion && (
                  <div className="sr-result-card" style={{animationDelay: '0.6s'}}>
                    <h3>Final Conclusion</h3>
                    <p style={{lineHeight: '1.8', fontSize: '18px'}}>{result.final_conclusion}</p>
                  </div>
                )}

                {result.actionable_next_steps && result.actionable_next_steps.length > 0 && (
                  <div className="sr-result-card" style={{animationDelay: '0.7s'}}>
                    <h3>Recommended Next Steps</h3>
                    <ul className="sr-next-steps">
                      {result.actionable_next_steps.map((step: string, idx: number) => (
                        <li key={idx}>{step}</li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>
            )}

            {result.status === 'ok' && result.html_report && (
              <div className="sr-results-content">
                <div className="sr-result-nav">
                  <button
                    className={`sr-button-secondary ${resultView === 'executive' ? 'active' : ''}`}
                    onClick={() => setResultView('executive')}
                  >
                    Executive Summary Report
                  </button>
                  <button
                    className={`sr-button-secondary ${resultView === 'results' ? 'active' : ''}`}
                    onClick={() => setResultView('results')}
                  >
                    Results Chapter Report
                  </button>
                  <button
                    className={`sr-button-secondary ${resultView === 'chat' ? 'active' : ''}`}
                    onClick={() => setResultView('chat')}
                  >
                    Chat with Gaply (Ask about your analysis)
                  </button>
                </div>

                {resultView === 'executive' && (
                  <div className="sr-result-card" style={{animationDelay: '0.1s'}}>
                    <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '24px'}}>
                      <div>
                        <h2 style={{fontSize: '24px', fontWeight: '700', marginBottom: '8px'}}>📄 Executive Summary Report</h2>
                        <p style={{fontSize: '13px', color: 'rgba(255, 255, 255, 0.7)'}}>
                          Clear summary, rationale, findings, and conclusion
                        </p>
                      </div>
                      <button
                        className="sr-button-primary"
                        style={{padding: '10px 20px', fontSize: '13px', fontWeight: '600'}}
                        onClick={() => {
                          const blob = new Blob([enhancedExecutiveReport], { type: 'text/html' });
                          const url = URL.createObjectURL(blob);
                          const a = document.createElement('a');
                          a.href = url;
                          a.download = `${title.replace(/\s+/g, '_')}_Executive_Summary_Report.html`;
                          a.click();
                          URL.revokeObjectURL(url);
                        }}
                      >
                        ⬇️ Download Report
                      </button>
                    </div>
                    <div className="sr-html-report">
                      <iframe
                        srcDoc={enhancedExecutiveReport}
                        title="Executive Summary Report"
                        className="sr-report-iframe"
                        style={{width: '100%', minHeight: '800px', border: 'none', borderRadius: '12px'}}
                      />
                    </div>
                  </div>
                )}

                {resultView === 'results' && (
                  <div className="sr-result-card" style={{animationDelay: '0.1s'}}>
                    <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '24px'}}>
                      <div>
                        <h2 style={{fontSize: '24px', fontWeight: '700', marginBottom: '8px'}}>📊 Results Chapter Report</h2>
                        <p style={{fontSize: '13px', color: 'rgba(255, 255, 255, 0.7)'}}>
                          All statistical tables and figures in sequence
                        </p>
                      </div>
                      <button
                        className="sr-button-primary"
                        style={{padding: '10px 20px', fontSize: '13px', fontWeight: '600'}}
                        onClick={() => {
                          const blob = new Blob([enhancedResultsReport], { type: 'text/html' });
                          const url = URL.createObjectURL(blob);
                          const a = document.createElement('a');
                          a.href = url;
                          a.download = `${title.replace(/\s+/g, '_')}_Results_Chapter_Report.html`;
                          a.click();
                          URL.revokeObjectURL(url);
                        }}
                      >
                        ⬇️ Download Report
                      </button>
                    </div>
                    <div className="sr-html-report">
                      <iframe
                        srcDoc={enhancedResultsReport}
                        title="Results Chapter Report"
                        className="sr-report-iframe"
                        style={{width: '100%', minHeight: '800px', border: 'none', borderRadius: '12px'}}
                      />
                    </div>
                  </div>
                )}

                {resultView === 'chat' && (
                  <div className="sr-result-card" style={{animationDelay: '0.2s'}}>
                    <h3>Chat with Gaply (Ask about your analysis)</h3>
                    <div className="sr-chat-panel">
                      <div className="sr-chat-messages">
                        {chatMessages.length === 0 && (
                          <div className="sr-chat-empty">Ask any question about your results, tables, or tests.</div>
                        )}
                        {chatMessages.map((msg, idx) => (
                          <div key={idx} className={`sr-chat-bubble ${msg.role}`}>
                            {msg.role === 'assistant' ? (
                              <div
                                className="sr-chat-content"
                                dangerouslySetInnerHTML={{ __html: msg.content }}
                              />
                            ) : (
                              msg.content
                            )}
                          </div>
                        ))}
                      </div>
                      <div className="sr-chat-input">
                        <input
                          type="text"
                          value={chatInput}
                          onChange={(e) => setChatInput(e.target.value)}
                          placeholder="Ask about your analysis..."
                          className="sr-input"
                        />
                        <button className="sr-button-primary" onClick={handleChatSubmit}>
                          Send
                        </button>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </section>
      )}
    </div>
  );
};

export default StatisticalResearchOrchestratorPage;
